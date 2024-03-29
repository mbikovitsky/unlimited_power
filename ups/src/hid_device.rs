use std::convert::TryInto;

use anyhow::{anyhow, Result};
use windows::{
    core::Interface,
    Devices::{
        Custom::{CustomDevice, DeviceAccessMode, DeviceSharingMode},
        Enumeration::{DeviceInformation, DeviceInformationCollection},
    },
    Storage::Streams::{Buffer, DataReader, IBuffer},
    Win32::System::WinRT::IMemoryBufferByteAccess,
};

use crate::util::slice_to_ibuffer;
use crate::{hid_util::HidInfo, util::ioctl_number_to_class};

#[derive(Debug)]
pub struct HidDevice {
    device: CustomDevice,
    input_report_size: usize,
    output_report_size: usize,
}

impl HidDevice {
    pub async fn new(
        usage_page: Option<u16>,
        usage_id: Option<u16>,
        vendor_id: u16,
        product_id: u16,
    ) -> Result<Self> {
        let devices = Self::get_devices(usage_page, usage_id, vendor_id, product_id).await?;
        assert_eq!(devices.Size()?, 1);

        let device_id: String = devices.GetAt(0)?.Id()?.try_into().unwrap();

        let caps = HidInfo::new(&device_id)?.preparsed_data()?.caps()?;
        let input_report_size = caps.InputReportByteLength;
        let output_report_size = caps.OutputReportByteLength;

        let device = Self::open_device(&device_id).await?;

        return Ok(HidDevice {
            device,
            input_report_size: input_report_size.into(),
            output_report_size: output_report_size.into(),
        });
    }

    async fn get_devices(
        usage_page: Option<u16>,
        usage_id: Option<u16>,
        vendor_id: u16,
        product_id: u16,
    ) -> Result<DeviceInformationCollection> {
        let selector = format!(
            concat!(
                "System.Devices.InterfaceClassGuid:=\"{{4D1E55B2-F16F-11CF-88CB-001111000030}}\"",
                " AND System.Devices.InterfaceEnabled:=System.StructuredQueryType.Boolean#True",
                "{}",
                "{}",
                " AND System.DeviceInterface.Hid.VendorId:={}",
                " AND System.DeviceInterface.Hid.ProductId:={}"
            ),
            if let Some(usage_page) = usage_page {
                format!(" AND System.DeviceInterface.Hid.UsagePage:={}", usage_page)
            } else {
                "".to_string()
            },
            if let Some(usage_id) = usage_id {
                format!(" AND System.DeviceInterface.Hid.UsageId:={}", usage_id)
            } else {
                "".to_string()
            },
            vendor_id,
            product_id
        );

        Ok(DeviceInformation::FindAllAsyncAqsFilter(&selector.into())?.await?)
    }

    async fn open_device(device_id: &str) -> Result<CustomDevice> {
        let future = CustomDevice::FromIdAsync(
            &device_id.into(),
            DeviceAccessMode::ReadWrite,
            DeviceSharingMode::Exclusive,
        )?;
        Ok(future.await?)
    }

    pub async fn send_output_report(&self, report_id: u8, data: &[u8]) -> Result<()> {
        let report = self.create_output_report(report_id, data)?;

        let future = {
            let report_buffer = slice_to_ibuffer(&report)?;
            self.device.OutputStream()?.WriteAsync(&report_buffer)?
        };
        let written = future.await?;
        assert_eq!(written, self.output_report_size.try_into().unwrap());

        Ok(())
    }

    fn create_output_report(&self, report_id: u8, data: &[u8]) -> Result<Vec<u8>> {
        assert!(self.output_report_size >= 1);
        if data.len() > self.output_report_size - 1 {
            return Err(anyhow!("Supplied data does not fit in report"));
        }

        let mut report = vec![0u8; self.output_report_size];
        report[0] = report_id;
        report[1..data.len() + 1].copy_from_slice(data);

        Ok(report)
    }

    pub async fn read_input_report(&self) -> Result<(u8, Vec<u8>)> {
        let reader = DataReader::CreateDataReader(&self.device.InputStream()?)?;

        let future = reader.LoadAsync(self.input_report_size.try_into().unwrap())?;
        future.await?;

        assert!(self.input_report_size >= 1);

        let report_id = reader.ReadByte()?;

        let mut report = vec![0u8; self.input_report_size - 1];
        reader.ReadBytes(&mut report)?;

        Ok((report_id, report))
    }

    pub async fn io_control(
        &self,
        control_code: u32,
        input_buffer: Option<&[u8]>,
        output_buffer: Option<&mut [u8]>,
    ) -> Result<u32> {
        let output_ibuffer = if let Some(output_buffer) = &output_buffer {
            Some(Buffer::Create(output_buffer.len().try_into().unwrap())?)
        } else {
            None
        };

        let result = {
            let future = self.device.SendIOControlAsync(
                &ioctl_number_to_class(control_code)?,
                input_buffer
                    .map(|input| slice_to_ibuffer(input))
                    .transpose()?
                    .as_ref(),
                output_ibuffer
                    .as_ref()
                    .map(|buffer| buffer.cast::<IBuffer>())
                    .transpose()?
                    .as_ref(),
            )?;
            future.await?
        };

        if let Some(output_buffer) = output_buffer {
            let output_ibuffer = output_ibuffer.unwrap().cast::<IBuffer>()?;

            let byte_access = Buffer::CreateMemoryBufferOverIBuffer(&output_ibuffer)?
                .CreateReference()?
                .cast::<IMemoryBufferByteAccess>()?;

            unsafe {
                let mut data = std::ptr::null_mut();
                let mut len = 0;
                byte_access.GetBuffer(&mut data, &mut len)?;

                let bytes = std::slice::from_raw_parts(data, len.try_into().unwrap());

                output_buffer.copy_from_slice(bytes);
            };
        }

        Ok(result)
    }

    pub async fn get_indexed_string(&self, index: u32) -> Result<String> {
        // https://docs.microsoft.com/en-us/windows-hardware/drivers/ddi/hidclass/ni-hidclass-ioctl_hid_get_indexed_string

        const IOCTL_HID_GET_INDEXED_STRING: u32 = 0x000B01E2;

        let input = index.to_le_bytes();
        let mut output = [0u8; 4093];
        let returned = self
            .io_control(
                IOCTL_HID_GET_INDEXED_STRING,
                Some(&input),
                Some(&mut output),
            )
            .await?;

        let output: Vec<_> = output[..returned.try_into().unwrap()]
            .chunks_exact(std::mem::size_of::<u16>())
            .map(|chunk| u16::from_le_bytes(chunk.try_into().unwrap()))
            .collect();

        // Output must contain at least a null-terminator
        assert_eq!(output.last().unwrap(), &0);

        Ok(String::from_utf16_lossy(&output[..output.len() - 1]))
    }
}

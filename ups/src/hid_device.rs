use std::convert::TryInto;

use windows::{
    runtime::{Error, Result},
    Devices::{
        Custom::{CustomDevice, DeviceAccessMode, DeviceSharingMode},
        Enumeration::{DeviceInformation, DeviceInformationCollection},
        HumanInterfaceDevice,
    },
    Storage::Streams::DataReader,
    Win32::Foundation::E_INVALIDARG,
};

use crate::hid_util::HidInfo;
use crate::util::slice_to_buffer;

#[derive(Debug)]
pub struct HidDevice {
    device: CustomDevice,
    input_report_size: usize,
    output_report_size: usize,
}

impl HidDevice {
    pub async fn new(
        usage_page: u16,
        usage_id: u16,
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
        usage_page: u16,
        usage_id: u16,
        vendor_id: u16,
        product_id: u16,
    ) -> Result<DeviceInformationCollection> {
        let future = {
            let selector = HumanInterfaceDevice::HidDevice::GetDeviceSelectorVidPid(
                usage_page, usage_id, vendor_id, product_id,
            )?;

            DeviceInformation::FindAllAsyncAqsFilter(selector)?
        };
        future.await
    }

    async fn open_device(device_id: &str) -> Result<CustomDevice> {
        let future = CustomDevice::FromIdAsync(
            device_id,
            DeviceAccessMode::ReadWrite,
            DeviceSharingMode::Exclusive,
        )?;
        future.await
    }

    pub async fn send_output_report(&self, report_id: u8, data: &[u8]) -> Result<()> {
        let report = self.create_output_report(report_id, data)?;

        let future = {
            let report_buffer = slice_to_buffer(&report)?;
            self.device.OutputStream()?.WriteAsync(report_buffer)?
        };
        let written = future.await?;
        assert_eq!(written, self.output_report_size.try_into().unwrap());

        Ok(())
    }

    fn create_output_report(&self, report_id: u8, data: &[u8]) -> Result<Vec<u8>> {
        assert!(self.output_report_size >= 1);
        if data.len() > self.output_report_size - 1 {
            return Err(Error::new(
                E_INVALIDARG,
                "Supplied data does not fit in report",
            ));
        }

        let mut report = vec![0u8; self.output_report_size];
        report[0] = report_id;
        report[1..data.len() + 1].copy_from_slice(data);

        Ok(report)
    }

    pub async fn read_input_report(&self) -> Result<(u8, Vec<u8>)> {
        let reader = DataReader::CreateDataReader(self.device.InputStream()?)?;

        let future = reader.LoadAsync(self.input_report_size.try_into().unwrap())?;
        future.await?;

        assert!(self.input_report_size >= 1);

        let report_id = reader.ReadByte()?;

        let mut report = vec![0u8; self.input_report_size - 1];
        reader.ReadBytes(&mut report)?;

        Ok((report_id, report))
    }
}

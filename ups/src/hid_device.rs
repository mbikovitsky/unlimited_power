use std::convert::TryInto;

use windows::{Error, ErrorCode, Result};

use bindings::windows::{
    devices::enumeration::DeviceInformation,
    devices::{
        custom::{CustomDevice, DeviceAccessMode, DeviceSharingMode},
        enumeration::DeviceInformationCollection,
        human_interface_device,
    },
    storage::streams::DataReader,
    win32::system_services::E_INVALIDARG,
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
        assert_eq!(devices.size()?, 1);

        let device_id: String = devices.get_at(0)?.id()?.try_into().unwrap();

        let caps = HidInfo::new(&device_id)?.preparsed_data()?.caps()?;
        let input_report_size = caps.input_report_byte_length;
        let output_report_size = caps.output_report_byte_length;

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
    ) -> windows::Result<DeviceInformationCollection> {
        let future = {
            let selector = human_interface_device::HidDevice::get_device_selector_vid_pid(
                usage_page, usage_id, vendor_id, product_id,
            )?;

            DeviceInformation::find_all_async_aqs_filter(selector)?
        };
        future.await
    }

    async fn open_device(device_id: &str) -> windows::Result<CustomDevice> {
        let future = CustomDevice::from_id_async(
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
            self.device.output_stream()?.write_async(report_buffer)?
        };
        let written = future.await?;
        assert_eq!(written, self.output_report_size.try_into().unwrap());

        Ok(())
    }

    fn create_output_report(&self, report_id: u8, data: &[u8]) -> Result<Vec<u8>> {
        assert!(self.output_report_size >= 1);
        if data.len() > self.output_report_size - 1 {
            return Err(Error::new(
                ErrorCode(E_INVALIDARG as u32),
                "Supplied data does not fit in report",
            ));
        }

        let mut report = vec![0u8; self.output_report_size];
        report[0] = report_id;
        report[1..data.len() + 1].copy_from_slice(data);

        Ok(report)
    }

    pub async fn read_input_report(&self) -> Result<(u8, Vec<u8>)> {
        let reader = DataReader::create_data_reader(self.device.input_stream()?)?;

        let future = reader.load_async(self.input_report_size.try_into().unwrap())?;
        future.await?;

        assert!(self.input_report_size >= 1);

        let report_id = reader.read_byte()?;

        let mut report = vec![0u8; self.input_report_size - 1];
        reader.read_bytes(&mut report)?;

        Ok((report_id, report))
    }
}

use std::time::Duration;

use bitflags::bitflags;
use tokio::{sync::Mutex, time::timeout};
use windows::{Error, ErrorCode, Result};

use bindings::windows::win32::system_services::{E_UNEXPECTED, OLE_E_CANTCONVERT, RPC_E_TIMEOUT};

use crate::hid_device::HidDevice;

const REPORT_ID: u8 = 0;

const HEADER: char = '(';
const TERMINATOR: char = '\r';

const SEND_TIMEOUT_MS: u64 = 1000;
const RECEIVE_TIMEOUT_MS: u64 = 250;
const RECEIVE_TOTAL_TIMEOUT_MS: u64 = 2400;

#[derive(Debug)]
pub struct HidUps {
    device: Mutex<HidDevice>,
}

impl HidUps {
    pub fn new(device: HidDevice) -> Result<Self> {
        Ok(Self {
            device: Mutex::new(device),
        })
    }

    pub async fn protocol(&self) -> Result<UpsProtocol> {
        let response = self.transact_command("M").await?;
        Ok(match response.as_str() {
            "P" => UpsProtocol::P,
            "T" => UpsProtocol::T,
            "V" => UpsProtocol::V,
            _ => UpsProtocol::Unknown,
        })
    }

    pub async fn status(&self) -> Result<UpsStatus> {
        match self.protocol().await? {
            UpsProtocol::V => {}
            _ => todo!("Protocol not implemented"),
        };

        let response = self.transact_command("QS").await?;

        match response.chars().nth(0) {
            Some(first_char) => {
                if first_char != HEADER {
                    return Err(Error::new(
                        ErrorCode(E_UNEXPECTED as u32),
                        "Unexpected QS response header",
                    ));
                }
            }
            None => {
                return Err(Error::new(
                    ErrorCode(E_UNEXPECTED as u32),
                    "QS response too short",
                ))
            }
        }
        assert!(HEADER.is_ascii());
        let response = &response[1..];

        let parts: Vec<_> = response.split_whitespace().collect();
        if parts.len() != 8 {
            return Err(Error::new(
                ErrorCode(E_UNEXPECTED as u32),
                "Unexpected number of QS response parts",
            ));
        }

        let status = UpsStatus {
            input_voltage: parts[0].parse().unwrap_or(f32::NAN),
            input_fault_voltage: parts[1].parse().unwrap_or(f32::NAN),
            output_voltage: parts[2].parse().unwrap_or(f32::NAN),
            output_load_level: parts[3].parse().unwrap_or(0),
            output_frequency: parts[4].parse().unwrap_or(f32::NAN),
            battery_voltage: parts[5].parse().unwrap_or(f32::NAN),
            internal_temperature: parts[6].parse().unwrap_or(f32::NAN),
            flags: UpsStatusFlags::from_bits(u8::from_str_radix(parts[7], 2).unwrap_or(0)).unwrap(),
        };

        Ok(status)
    }

    async fn transact_command(&self, command: &str) -> Result<String> {
        let device = self.device.lock().await;
        Self::send_command(&*device, command).await?;
        let response = Self::read_response(&*device).await?;

        Ok(response)
    }

    async fn send_command(device: &HidDevice, command: &str) -> Result<()> {
        assert!(TERMINATOR.is_ascii());

        let mut command = command.to_string();
        command.push(TERMINATOR);

        let future = device.send_output_report(REPORT_ID, command.as_bytes());
        let future = timeout(Duration::from_millis(SEND_TIMEOUT_MS), future);
        match future.await {
            Ok(result) => result?,
            Err(_) => {
                return Err(Error::new(
                    ErrorCode(RPC_E_TIMEOUT as u32),
                    "Sending command timed-out",
                ))
            }
        };

        Ok(())
    }

    async fn read_response(device: &HidDevice) -> Result<String> {
        let future = Self::read_all_response_packets(device);
        let future = timeout(Duration::from_millis(RECEIVE_TOTAL_TIMEOUT_MS), future);
        let response = match future.await {
            Ok(result) => result?,
            Err(_) => {
                return Err(Error::new(
                    ErrorCode(RPC_E_TIMEOUT as u32),
                    "Receiving response timed-out",
                ))
            }
        };

        let response = match String::from_utf8(response) {
            Ok(response) => response,
            Err(_) => {
                return Err(Error::new(
                    ErrorCode(OLE_E_CANTCONVERT as u32),
                    "UPS response is not valid UTF-8",
                ))
            }
        };
        let response = &response[0..response.find(TERMINATOR).unwrap()];

        Ok(response.to_string())
    }

    async fn read_all_response_packets(device: &HidDevice) -> Result<Vec<u8>> {
        assert!(TERMINATOR.is_ascii());

        let mut response: Vec<u8> = Vec::new();
        loop {
            let packet = Self::read_single_response_packet(device).await?;

            response.extend(&packet);

            if packet
                .iter()
                .find(|&&elem| elem == TERMINATOR as u8)
                .is_some()
            {
                break;
            }
        }

        Ok(response)
    }

    async fn read_single_response_packet(device: &HidDevice) -> Result<Vec<u8>> {
        let future = device.read_input_report();
        let future = timeout(Duration::from_millis(RECEIVE_TIMEOUT_MS), future);
        let (report_id, report) = match future.await {
            Ok(result) => result?,
            Err(_) => {
                return Err(Error::new(
                    ErrorCode(RPC_E_TIMEOUT as u32),
                    "Receiving response timed-out",
                ))
            }
        };

        if report_id != REPORT_ID {
            return Err(Error::new(
                ErrorCode(E_UNEXPECTED as u32),
                "Unexpected HID report ID",
            ));
        }

        Ok(report)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsProtocol {
    P,
    T,
    V,
    Unknown,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UpsStatus {
    input_voltage: f32,
    input_fault_voltage: f32,
    output_voltage: f32,
    output_load_level: u32,
    output_frequency: f32,
    battery_voltage: f32,
    internal_temperature: f32,
    flags: UpsStatusFlags,
}

impl UpsStatus {
    pub fn flags(&self) -> UpsStatusFlags {
        self.flags
    }

    pub fn work_mode(&self) -> UpsWorkMode {
        if self.flags.contains(UpsStatusFlags::UPS_FAULT) {
            UpsWorkMode::Fault
        } else if self.flags.contains(UpsStatusFlags::UTILITY_FAIL) {
            UpsWorkMode::Battery
        } else if self.flags.contains(UpsStatusFlags::SELF_TEST_IN_PROGRESS) {
            UpsWorkMode::BatteryTest
        } else {
            UpsWorkMode::Line
        }
    }
}

bitflags! {
    #[derive(Default)]
    pub struct UpsStatusFlags: u8 {
        const BEEPER_ACTIVE         = 0b00000001;
        const UPS_SHUTDOWN_ACTIVE   = 0b00000010;
        const SELF_TEST_IN_PROGRESS = 0b00000100;
        const UPS_LINE_INTERACTIVE  = 0b00001000;
        const UPS_FAULT             = 0b00010000;
        const BOOST_OR_BUCK_MODE    = 0b00100000;
        const BATTERY_LOW           = 0b01000000;
        const UTILITY_FAIL          = 0b10000000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpsWorkMode {
    Line,
    Battery,
    BatteryTest,
    Fault,
}

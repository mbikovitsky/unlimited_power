use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::{sync::Mutex, time::timeout};

use crate::{
    hid_device::HidDevice,
    ups::{Ups, UpsStatus, UpsStatusFlags},
};

const REPORT_ID: u8 = 0;

const HEADER: char = '(';
const TERMINATOR: char = '\r';

const SEND_TIMEOUT_MS: u64 = 1000;
const RECEIVE_TIMEOUT_MS: u64 = 250;
const RECEIVE_TOTAL_TIMEOUT_MS: u64 = 2400;

#[derive(Debug)]
pub struct VoltronicHidUps {
    device: Mutex<HidDevice>,
}

impl VoltronicHidUps {
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
            Err(_) => return Err(anyhow!("Sending command timed-out")),
        };

        Ok(())
    }

    async fn read_response(device: &HidDevice) -> Result<String> {
        let future = Self::read_all_response_packets(device);
        let future = timeout(Duration::from_millis(RECEIVE_TOTAL_TIMEOUT_MS), future);
        let response = match future.await {
            Ok(result) => result?,
            Err(_) => return Err(anyhow!("Receiving response timed-out")),
        };

        let response = match String::from_utf8(response) {
            Ok(response) => response,
            Err(_) => return Err(anyhow!("UPS response is not valid UTF-8")),
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
            Err(_) => return Err(anyhow!("Receiving response timed-out")),
        };

        if report_id != REPORT_ID {
            return Err(anyhow!("Unexpected HID report ID"));
        }

        Ok(report)
    }
}

#[async_trait]
impl Ups for VoltronicHidUps {
    async fn status(&self) -> Result<UpsStatus> {
        match self.protocol().await? {
            UpsProtocol::V => {}
            _ => todo!("Protocol not implemented"),
        };

        let response = self.transact_command("QS").await?;

        match response.chars().nth(0) {
            Some(first_char) => {
                if first_char != HEADER {
                    return Err(anyhow!("Unexpected QS response header"));
                }
            }
            None => return Err(anyhow!("QS response too short")),
        }
        assert!(HEADER.is_ascii());
        let response = &response[1..];

        let parts: Vec<_> = response.split_whitespace().collect();
        if parts.len() != 8 {
            return Err(anyhow!("Unexpected number of QS response parts"));
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsProtocol {
    P,
    T,
    V,
    Unknown,
}

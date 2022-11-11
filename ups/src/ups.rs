use std::str::FromStr;

use anyhow::{bail, Result};
use async_trait::async_trait;
use bitflags::bitflags;

#[async_trait]
pub trait Ups {
    /// Get UPS status
    async fn status(&self) -> Result<UpsStatus>;

    /// Turn the beeper on or off
    async fn beeper(&self, on: bool) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UpsStatus {
    pub input_voltage: f32,
    pub input_fault_voltage: f32,
    pub output_voltage: f32,
    pub output_load_level: u32,
    pub output_frequency: f32,
    pub battery_voltage: f32,
    pub internal_temperature: f32,
    pub flags: UpsStatusFlags,
}

impl UpsStatus {
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

impl FromStr for UpsStatus {
    type Err = anyhow::Error;

    fn from_str(string: &str) -> Result<Self> {
        const HEADER: char = '(';
        const TERMINATOR: char = '\r';

        match string.chars().nth(0) {
            Some(first_char) => {
                if first_char != HEADER {
                    bail!("Unexpected status string header");
                }
            }
            None => bail!("Status string too short"),
        }

        match string.chars().last() {
            Some(last_char) => {
                if last_char != TERMINATOR {
                    bail!("Unexpected status string terminator");
                }
            }
            None => bail!("Status string too short"),
        }

        assert!(HEADER.is_ascii());
        assert!(TERMINATOR.is_ascii());
        let string = &string[1..string.len() - 1];

        let parts: Vec<_> = string.split_whitespace().collect();
        if parts.len() != 8 {
            bail!("Unexpected number of status string parts");
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

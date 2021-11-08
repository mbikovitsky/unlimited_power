use anyhow::Result;
use async_trait::async_trait;
use bitflags::bitflags;

#[async_trait]
pub trait Ups {
    async fn status(&self) -> Result<UpsStatus>;
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

use std::convert::TryInto;

use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeConfig {
    pub hibernate: bool,
    pub poll_interval_ms: u32,
    pub poll_failure_timeout_ms: u32,
    pub shutdown_timeout_s: u32,
    pub hid_usage_page: Option<u16>,
    pub hid_usage_id: Option<u16>,
    pub vendor_id: u16,
    pub product_id: u16,
}

impl RuntimeConfig {
    pub fn read() -> anyhow::Result<Self> {
        let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(Self::registry_path())?;

        let hibernate = key.get_value("hibernate").map(|value: u32| value != 0)?;
        let poll_interval_ms: u32 = key.get_value("poll_interval_ms")?;
        let poll_failure_timeout_ms: u32 = key.get_value("poll_failure_timeout_ms")?;
        let shutdown_timeout_s: u32 = key.get_value("shutdown_timeout_s")?;
        let hid_usage_page: Option<u32> = key.get_value("hid_usage_page").ok();
        let hid_usage_id: Option<u32> = key.get_value("hid_usage_id").ok();
        let vendor_id: u32 = key.get_value("vendor_id")?;
        let product_id: u32 = key.get_value("product_id")?;

        Ok(Self {
            hibernate,
            poll_interval_ms,
            poll_failure_timeout_ms,
            shutdown_timeout_s,
            hid_usage_page: hid_usage_page.map(u32::try_into).transpose()?,
            hid_usage_id: hid_usage_id.map(u32::try_into).transpose()?,
            vendor_id: vendor_id.try_into()?,
            product_id: product_id.try_into()?,
        })
    }

    pub fn write(&self) -> anyhow::Result<()> {
        let (key, _) = RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey(Self::registry_path())?;

        key.set_value("hibernate", if self.hibernate { &1u32 } else { &0u32 })?;
        key.set_value("poll_interval_ms", &self.poll_interval_ms)?;
        key.set_value("poll_failure_timeout_ms", &self.poll_failure_timeout_ms)?;
        key.set_value("shutdown_timeout_s", &self.shutdown_timeout_s)?;

        if let Some(hid_usage_page) = self.hid_usage_page {
            let hid_usage_page: u32 = hid_usage_page.into();
            key.set_value("hid_usage_page", &hid_usage_page)?;
        } else {
            key.delete_value("hid_usage_page")?;
        }

        if let Some(hid_usage_id) = self.hid_usage_id {
            let hid_usage_id: u32 = hid_usage_id.into();
            key.set_value("hid_usage_id", &hid_usage_id)?;
        } else {
            key.delete_value("hid_usage_id")?;
        }

        let vendor_id: u32 = self.vendor_id.into();
        key.set_value("vendor_id", &vendor_id)?;

        let product_id: u32 = self.product_id.into();
        key.set_value("product_id", &product_id)?;

        Ok(())
    }

    fn registry_path() -> String {
        format!(
            r"SYSTEM\CurrentControlSet\Services\{}\Parameters",
            HardCodedConfig::SERVICE_NAME
        )
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            hibernate: true,
            poll_interval_ms: 1000,
            poll_failure_timeout_ms: 10000,
            shutdown_timeout_s: 5 * 60,
            hid_usage_page: Some(0xFF00),
            hid_usage_id: Some(0x0001),
            vendor_id: 0x0665,
            product_id: 0x5161,
        }
    }
}

pub(crate) struct HardCodedConfig;

impl HardCodedConfig {
    pub const SERVICE_NAME: &'static str = "unlimited_power";

    pub const SERVICE_DISPLAY_NAME: &'static str = "Unlimited Power";

    pub const MAX_START_TIME_MS: u32 = 3000;

    pub const MAX_STOP_TIME_MS: u32 = 1000;
}

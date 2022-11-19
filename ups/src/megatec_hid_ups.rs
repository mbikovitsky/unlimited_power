use anyhow::Result;
use async_trait::async_trait;

use crate::{
    hid_device::HidDevice,
    ups::{Ups, UpsStatus},
};

#[derive(Debug)]
pub struct MegatecHidUps {
    device: HidDevice,
}

impl MegatecHidUps {
    pub fn new(device: HidDevice) -> Result<Self> {
        Ok(Self { device })
    }
}

#[async_trait]
impl Ups for MegatecHidUps {
    async fn status(&self) -> Result<UpsStatus> {
        let status_string = self.device.get_indexed_string(3).await?;

        Ok(status_string.parse()?)
    }

    async fn beeper_toggle(&self) -> Result<()> {
        self.device.get_indexed_string(7).await?;
        Ok(())
    }
}

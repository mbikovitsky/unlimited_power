use windows::{
    runtime::Result,
    Win32::Security::{ImpersonateSelf, RevertToSelf, SECURITY_IMPERSONATION_LEVEL},
};

#[must_use]
pub struct SelfImpersonator;

impl SelfImpersonator {
    pub fn impersonate(impersonation_level: SECURITY_IMPERSONATION_LEVEL) -> Result<Self> {
        unsafe {
            ImpersonateSelf(impersonation_level).ok()?;
        }
        Ok(Self {})
    }
}

impl Drop for SelfImpersonator {
    fn drop(&mut self) {
        unsafe {
            RevertToSelf().expect("RevertToSelf failed");
        }
    }
}

impl !Send for SelfImpersonator {}
impl !Sync for SelfImpersonator {}

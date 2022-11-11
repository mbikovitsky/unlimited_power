use std::marker::PhantomData;

use static_assertions::assert_not_impl_all;
use windows::{
    core::Result,
    Win32::Security::{ImpersonateSelf, RevertToSelf, SECURITY_IMPERSONATION_LEVEL},
};

#[must_use]
pub struct SelfImpersonator(PhantomData<*const ()>);

impl SelfImpersonator {
    pub fn impersonate(impersonation_level: SECURITY_IMPERSONATION_LEVEL) -> Result<Self> {
        unsafe {
            ImpersonateSelf(impersonation_level).ok()?;
        }
        Ok(Self(PhantomData))
    }
}

assert_not_impl_all!(SelfImpersonator: Send, Sync);

impl Drop for SelfImpersonator {
    fn drop(&mut self) {
        unsafe {
            RevertToSelf().expect("RevertToSelf failed");
        }
    }
}

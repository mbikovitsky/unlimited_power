use std::path::Path;

use widestring::U16CString;
use windows::{
    runtime::{Error, Result},
    Win32::{
        Foundation::PWSTR,
        Security::SC_HANDLE,
        System::Services::{
            ChangeServiceConfig2W, CloseServiceHandle, CreateServiceW, DeleteService,
            OpenSCManagerW, OpenServiceW, ENUM_SERVICE_TYPE,
            SERVICE_CONFIG_REQUIRED_PRIVILEGES_INFO, SERVICE_ERROR,
            SERVICE_REQUIRED_PRIVILEGES_INFOW, SERVICE_START_TYPE,
        },
    },
};

#[derive(Debug)]
pub struct ScManager {
    handle: SC_HANDLE,
}

impl ScManager {
    pub fn open_local(desired_access: ScManagerAccessRights) -> Result<Self> {
        let handle = unsafe { OpenSCManagerW(None, None, desired_access.0) };
        if handle.0 == 0 {
            return Err(Error::from_win32());
        }
        Ok(Self { handle })
    }

    pub fn create_local_system_service(
        &self,
        service_name: impl AsRef<str>,
        display_name: impl AsRef<str>,
        service_type: ENUM_SERVICE_TYPE,
        start_type: SERVICE_START_TYPE,
        error_control: SERVICE_ERROR,
        binary_path: impl AsRef<Path>,
    ) -> Result<Service> {
        let handle = unsafe {
            CreateServiceW(
                self.handle,
                service_name.as_ref(),
                display_name.as_ref(),
                ServiceAccessRights::SERVICE_ALL_ACCESS.0,
                service_type,
                start_type,
                error_control,
                binary_path.as_ref().to_str().unwrap(),
                None,
                std::ptr::null_mut(),
                None,
                None,
                None,
            )
        };
        if handle.0 == 0 {
            return Err(Error::from_win32());
        }
        Ok(Service { handle })
    }

    pub fn open_service(
        &self,
        service_name: impl AsRef<str>,
        desired_access: ServiceAccessRights,
    ) -> Result<Service> {
        let handle = unsafe { OpenServiceW(self.handle, service_name.as_ref(), desired_access.0) };
        if handle.0 == 0 {
            return Err(Error::from_win32());
        }
        Ok(Service { handle })
    }
}

impl Drop for ScManager {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.handle).expect("CloseServiceHandle failed");
        }
    }
}

unsafe impl Send for ScManager {}
impl !Sync for ScManager {}

#[derive(Debug)]
pub struct Service {
    handle: SC_HANDLE,
}

impl Service {
    pub fn delete(&self) -> Result<()> {
        unsafe { DeleteService(self.handle).ok() }
    }

    pub fn set_required_privileges<I, T>(&self, privileges: I) -> Result<()>
    where
        I: IntoIterator<Item = T>,
        T: AsRef<str>,
    {
        let mut multi_string: Vec<_> = privileges
            .into_iter()
            .map(|privilege| U16CString::from_str(privilege).unwrap())
            .chain(std::iter::once(U16CString::from_str("").unwrap()))
            .map(|string| string.into_vec_with_nul())
            .flatten()
            .collect();

        let mut info = SERVICE_REQUIRED_PRIVILEGES_INFOW {
            pmszRequiredPrivileges: PWSTR(multi_string.as_mut_ptr()),
        };
        let info_ptr: *mut _ = &mut info;

        unsafe {
            ChangeServiceConfig2W(
                self.handle,
                SERVICE_CONFIG_REQUIRED_PRIVILEGES_INFO,
                info_ptr as _,
            )
            .ok()?;
        }

        Ok(())
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.handle).expect("CloseServiceHandle failed");
        }
    }
}

unsafe impl Send for Service {}
impl !Sync for Service {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScManagerAccessRights(u32);

#[allow(dead_code)]
impl ScManagerAccessRights {
    pub const SC_MANAGER_ALL_ACCESS: Self = Self(0xF003F);
    pub const SC_MANAGER_CREATE_SERVICE: Self = Self(0x0002);
    pub const SC_MANAGER_CONNECT: Self = Self(0x0001);
    pub const SC_MANAGER_ENUMERATE_SERVICE: Self = Self(0x0001);
    pub const SC_MANAGER_LOCK: Self = Self(0x0008);
    pub const SC_MANAGER_MODIFY_BOOT_CONFIG: Self = Self(0x0020);
    pub const SC_MANAGER_QUERY_LOCK_STATUS: Self = Self(0x0010);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceAccessRights(u32);

#[allow(dead_code)]
impl ServiceAccessRights {
    pub const SERVICE_ALL_ACCESS: Self = Self(0xF01FF);
    pub const SERVICE_CHANGE_CONFIG: Self = Self(0x0002);
    pub const SERVICE_ENUMERATE_DEPENDENTS: Self = Self(0x0008);
    pub const SERVICE_INTERROGATE: Self = Self(0x0080);
    pub const SERVICE_PAUSE_CONTINUE: Self = Self(0x0040);
    pub const SERVICE_QUERY_CONFIG: Self = Self(0x0001);
    pub const SERVICE_QUERY_STATUS: Self = Self(0x0004);
    pub const SERVICE_START: Self = Self(0x0010);
    pub const SERVICE_STOP: Self = Self(0x0020);
    pub const SERVICE_USER_DEFINED_CONTROL: Self = Self(0x0100);
    pub const DELETE: Self = Self(0x10000);
}

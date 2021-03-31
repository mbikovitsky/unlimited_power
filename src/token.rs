use widestring::U16CString;
use windows::ErrorCode;

use bindings::windows::win32::{
    security::{LookupPrivilegeValueW, OpenThreadToken, LUID_AND_ATTRIBUTES},
    system_services::{GetCurrentThread, BOOL, ERROR_NOT_ALL_ASSIGNED, HANDLE, PWSTR},
    windows_programming::CloseHandle,
};

#[derive(Debug)]
pub struct Token {
    handle: HANDLE,
}

impl Token {
    pub fn open_thread_token(desired_access: u32, open_as_self: bool) -> windows::Result<Self> {
        let mut thread_token = HANDLE::default();
        unsafe {
            OpenThreadToken(
                GetCurrentThread(),
                desired_access,
                open_as_self,
                &mut thread_token,
            )
            .ok()?;
        }
        Ok(Self {
            handle: thread_token,
        })
    }

    pub fn enable_privilege(&self, name: &str) -> windows::Result<()> {
        const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;

        let mut privileges = TOKEN_PRIVILEGES {
            privilege_count: 1,
            privileges: [LUID_AND_ATTRIBUTES {
                attributes: SE_PRIVILEGE_ENABLED,
                ..Default::default()
            }],
        };

        unsafe {
            LookupPrivilegeValueW(
                PWSTR::default(),
                PWSTR(
                    U16CString::from_str(name)
                        .unwrap()
                        .into_vec_with_nul()
                        .as_mut_ptr(),
                ),
                &mut privileges.privileges[0].luid,
            )
            .ok()?;
        }

        unsafe {
            AdjustTokenPrivileges(
                self.handle,
                false.into(),
                &mut privileges,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
            .ok()?;
        }
        if ErrorCode::from_win32(ERROR_NOT_ALL_ASSIGNED) == ErrorCode::from_thread() {
            return Err(windows::Error::new(
                ErrorCode::from_win32(ERROR_NOT_ALL_ASSIGNED),
                "AdjustTokenPrivileges",
            ));
        }

        Ok(())
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle).expect("CloseHandle failed");
        }
    }
}

unsafe impl Send for Token {}
impl !Sync for Token {}

extern "system" {
    #[link(name = "advapi32")]
    fn AdjustTokenPrivileges(
        token_handle: HANDLE,
        disable_all_privileges: BOOL,
        new_state: *mut TOKEN_PRIVILEGES,
        buffer_length: u32,
        previous_state: *mut TOKEN_PRIVILEGES,
        return_length: *mut u32,
    ) -> BOOL;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct TOKEN_PRIVILEGES {
    privilege_count: u32,
    privileges: [LUID_AND_ATTRIBUTES; 1],
}

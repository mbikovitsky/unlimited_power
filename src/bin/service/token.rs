use windows::{
    runtime::{Error, Result, HRESULT},
    Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_NOT_ALL_ASSIGNED, HANDLE},
        Security::{
            AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES,
            SE_PRIVILEGE_ENABLED, TOKEN_ACCESS_MASK, TOKEN_PRIVILEGES,
        },
        System::Threading::{GetCurrentThread, OpenThreadToken},
    },
};

#[derive(Debug)]
pub struct Token {
    handle: HANDLE,
}

impl Token {
    pub fn open_thread_token(
        desired_access: TOKEN_ACCESS_MASK,
        open_as_self: bool,
    ) -> Result<Self> {
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

    pub fn enable_privilege(&self, name: &str) -> Result<()> {
        let mut privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Attributes: SE_PRIVILEGE_ENABLED,
                ..Default::default()
            }],
        };

        unsafe {
            LookupPrivilegeValueW(None, name, &mut privileges.Privileges[0].Luid).ok()?;
        }

        unsafe {
            AdjustTokenPrivileges(
                self.handle,
                false,
                &mut privileges,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
            .ok()?;
        }
        if unsafe { GetLastError() } == ERROR_NOT_ALL_ASSIGNED {
            return Err(Error::new(
                HRESULT::from(ERROR_NOT_ALL_ASSIGNED),
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

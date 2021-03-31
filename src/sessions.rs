use std::{
    convert::TryInto,
    mem::{size_of, size_of_val},
    slice,
};

use widestring::U16CString;

use bindings::windows::win32::{
    remote_desktop_services::{
        WTSCloseServer, WTSEnumerateSessionsExW, WTSFreeMemoryExW, WTSSendMessageW,
        WTS_SESSION_INFO_1W, WTS_TYPE_CLASS,
    },
    system_services::{HANDLE, PWSTR},
};

pub use bindings::windows::win32::remote_desktop_services::WTS_CONNECTSTATE_CLASS;

#[derive(Debug)]
pub struct WTSServer {
    handle: HANDLE,
}

impl WTSServer {
    const WTS_CURRENT_SERVER_HANDLE: HANDLE = HANDLE(0);

    pub fn open_local() -> Self {
        Self {
            handle: Self::WTS_CURRENT_SERVER_HANDLE,
        }
    }

    pub fn sessions(&self) -> windows::Result<WTSSessionInfoList> {
        let mut sessions = std::ptr::null_mut();
        let mut count = 0;
        unsafe {
            let mut level = 1u32;
            WTSEnumerateSessionsExW(self.handle, &mut level, 0, &mut sessions, &mut count).ok()?;
        }
        Ok(WTSSessionInfoList { sessions, count })
    }

    pub fn send_message(
        &self,
        session_id: u32,
        title: impl AsRef<str>,
        message: impl AsRef<str>,
        style: u32,
    ) -> windows::Result<()> {
        let mut title = U16CString::from_str(title).unwrap().into_vec_with_nul();
        let mut message = U16CString::from_str(message).unwrap().into_vec_with_nul();

        let title_length = size_of_val(title.as_slice()) - size_of::<u16>();
        let message_length = size_of_val(message.as_slice()) - size_of::<u16>();

        let mut response = 0;
        unsafe {
            WTSSendMessageW(
                self.handle,
                session_id,
                PWSTR(title.as_mut_ptr()),
                title_length.try_into().unwrap(),
                PWSTR(message.as_mut_ptr()),
                message_length.try_into().unwrap(),
                style,
                0,
                &mut response,
                false,
            )
            .ok()?;
        }

        const IDASYNC: u32 = 0x7D01;
        debug_assert_eq!(response, IDASYNC);

        Ok(())
    }
}

impl Drop for WTSServer {
    fn drop(&mut self) {
        if self.handle != Self::WTS_CURRENT_SERVER_HANDLE {
            unsafe {
                WTSCloseServer(self.handle);
            }
        }
    }
}

unsafe impl Send for WTSServer {}
impl !Sync for WTSServer {}

#[derive(Debug)]
pub struct WTSSessionInfoList {
    sessions: *const WTS_SESSION_INFO_1W,
    count: u32,
}

impl WTSSessionInfoList {
    pub fn iter<'a>(&'a self) -> WTSSessionInfoIterator<'a> {
        WTSSessionInfoIterator {
            sessions: unsafe {
                slice::from_raw_parts(self.sessions, self.count.try_into().unwrap())
            },
            index: 0,
        }
    }
}

impl Drop for WTSSessionInfoList {
    fn drop(&mut self) {
        unsafe {
            WTSFreeMemoryExW(
                WTS_TYPE_CLASS::WTSTypeSessionInfoLevel1,
                self.sessions as _,
                self.count,
            )
            .expect("WTSFreeMemoryExW failed");
        }
    }
}

unsafe impl Send for WTSSessionInfoList {}
unsafe impl Sync for WTSSessionInfoList {}

#[derive(Debug)]
pub struct WTSSessionInfoIterator<'a> {
    sessions: &'a [WTS_SESSION_INFO_1W],
    index: usize,
}

impl<'a> Iterator for WTSSessionInfoIterator<'a> {
    type Item = WTSSessionInfo<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.sessions.len() {
            None
        } else {
            let info = &self.sessions[self.index];
            self.index += 1;
            Some(WTSSessionInfo { info })
        }
    }
}

#[derive(Debug)]
pub struct WTSSessionInfo<'a> {
    info: &'a WTS_SESSION_INFO_1W,
}

impl<'a> WTSSessionInfo<'a> {
    pub fn session_id(&self) -> u32 {
        self.info.session_id
    }

    pub fn is_local_session(&self) -> bool {
        // https://docs.microsoft.com/en-us/windows/win32/api/wtsapi32/ns-wtsapi32-wts_session_info_1a
        self.info.p_host_name.0.is_null()
    }

    pub fn connection_state(&self) -> WTS_CONNECTSTATE_CLASS {
        self.info.state
    }
}

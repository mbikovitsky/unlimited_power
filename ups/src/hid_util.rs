use windows::{
    runtime::{Error, Result},
    Win32::{
        Devices::HumanInterfaceDevice::{
            HidD_FreePreparsedData, HidD_GetPreparsedData, HidP_GetCaps, HIDP_CAPS,
        },
        Foundation::{CloseHandle, BOOL, HANDLE},
        Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        },
    },
};

#[derive(Debug)]
pub(crate) struct HidInfo {
    handle: HANDLE,
}

impl HidInfo {
    pub fn new(device_id: &str) -> Result<Self> {
        let handle = unsafe {
            CreateFileW(
                device_id,
                FILE_GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                HANDLE(0),
            )
        };
        let error = Error::from_win32().code();
        if let HANDLE(-1) = handle {
            return Err(Error::new(error, "CreateFileW"));
        }
        Ok(Self { handle })
    }

    pub fn preparsed_data(&self) -> Result<HidPreparsedData> {
        unsafe {
            let mut data = 0;
            let result = HidD_GetPreparsedData(self.handle, &mut data);
            let error = Error::from_win32().code();
            if result.0 == 0 {
                return Err(Error::new(error, "HidD_GetPreparsedData"));
            }
            Ok(HidPreparsedData { data })
        }
    }
}

impl Drop for HidInfo {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle).expect("CloseHandle failed");
        }
    }
}

unsafe impl Send for HidInfo {}
impl !Sync for HidInfo {}

#[derive(Debug)]
pub(crate) struct HidPreparsedData {
    data: isize,
}

impl HidPreparsedData {
    pub fn caps(&self) -> Result<HIDP_CAPS> {
        let mut caps = HIDP_CAPS::default();
        unsafe { HidP_GetCaps(self.data, &mut caps)? };
        Ok(caps)
    }
}

impl Drop for HidPreparsedData {
    fn drop(&mut self) {
        unsafe {
            let result = HidD_FreePreparsedData(self.data);
            let result = BOOL::from(result.0 != 0);
            result.expect("HidD_FreePreparsedData failed");
        }
    }
}

unsafe impl Send for HidPreparsedData {}
impl !Sync for HidPreparsedData {}

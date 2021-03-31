use widestring::U16CString;
use windows::{Error, ErrorCode, Result};

use bindings::windows::win32::{
    file_system::{
        CreateFileW, FILE_ACCESS_FLAGS, FILE_CREATE_FLAGS, FILE_FLAGS_AND_ATTRIBUTES,
        FILE_SHARE_FLAGS,
    },
    hid::{HidD_FreePreparsedData, HidD_GetPreparsedData},
    system_services::{BOOL, E_UNEXPECTED, HANDLE, NTSTATUS, PWSTR},
    windows_programming::CloseHandle,
};

#[derive(Debug)]
pub(crate) struct HidInfo {
    handle: HANDLE,
}

impl HidInfo {
    pub fn new(device_id: &str) -> Result<Self> {
        let handle = unsafe {
            CreateFileW(
                PWSTR(
                    U16CString::from_str(device_id)
                        .unwrap()
                        .into_vec_with_nul()
                        .as_mut_ptr(),
                ),
                FILE_ACCESS_FLAGS::FILE_GENERIC_READ,
                FILE_SHARE_FLAGS(
                    FILE_SHARE_FLAGS::FILE_SHARE_READ.0 | FILE_SHARE_FLAGS::FILE_SHARE_WRITE.0,
                ),
                std::ptr::null_mut(),
                FILE_CREATE_FLAGS::OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                HANDLE(0),
            )
        };
        let error = ErrorCode::from_thread();
        if let HANDLE(-1) = handle {
            return Err(Error::new(error, "CreateFileW"));
        }
        Ok(Self { handle })
    }

    pub fn preparsed_data(&self) -> Result<HidPreparsedData> {
        unsafe {
            let mut data = 0;
            let result = HidD_GetPreparsedData(self.handle, &mut data);
            let error = ErrorCode::from_thread();
            if result == 0 {
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
        const HIDP_STATUS_SUCCESS: NTSTATUS = NTSTATUS(0x00110000);

        let mut caps = HIDP_CAPS::default();
        let result = unsafe { HidP_GetCaps(self.data, &mut caps) };
        if result != HIDP_STATUS_SUCCESS {
            return Err(Error::new(ErrorCode(E_UNEXPECTED as u32), "HidP_GetCaps"));
        }
        Ok(caps)
    }
}

impl Drop for HidPreparsedData {
    fn drop(&mut self) {
        unsafe {
            let result = HidD_FreePreparsedData(self.data);
            let result = BOOL::from(result != 0);
            result.expect("HidD_FreePreparsedData failed");
        }
    }
}

unsafe impl Send for HidPreparsedData {}
impl !Sync for HidPreparsedData {}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct HIDP_CAPS {
    pub usage: u16,
    pub usage_page: u16,
    pub input_report_byte_length: u16,
    pub output_report_byte_length: u16,
    pub feature_report_byte_length: u16,
    reserved: [u16; 17],
    pub number_link_collection_nodes: u16,
    pub number_input_button_caps: u16,
    pub number_input_value_caps: u16,
    pub number_input_data_indices: u16,
    pub number_output_button_caps: u16,
    pub number_output_value_caps: u16,
    pub number_output_data_indices: u16,
    pub number_feature_button_caps: u16,
    pub number_feature_value_caps: u16,
    pub number_feature_data_indices: u16,
}

extern "system" {
    #[link(name = "hid")]
    fn HidP_GetCaps(preparsed_data: isize, capabilities: *mut HIDP_CAPS) -> NTSTATUS;
}

use widestring::U16CString;

use bindings::windows::win32::{debug::OutputDebugStringW, system_services::PWSTR};

pub(crate) static LOGGER: Logger = Logger;

pub(crate) struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if let Ok(wide_string) = U16CString::from_str(format!(
            "[{}] - [{}] - [{}] - [{}] - {}\n",
            record.target(),
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or(0),
            record.level(),
            record.args()
        )) {
            unsafe {
                OutputDebugStringW(PWSTR(wide_string.as_ptr() as _));
            }
        }
    }

    fn flush(&self) {}
}

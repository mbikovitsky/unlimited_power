use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;

pub(crate) static LOGGER: Logger = Logger;

pub(crate) struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let string = format!(
            "[{}] - [{}] - [{}] - [{}] - {}\n",
            record.target(),
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or(0),
            record.level(),
            record.args()
        );
        unsafe {
            OutputDebugStringW(string);
        }
    }

    fn flush(&self) {}
}

fn main() {
    windows::build!(
        windows::foundation::{
            IAsyncOperation,
            IAsyncOperationWithProgress,
        },

        windows::devices::human_interface_device::HidDevice,

        windows::devices::custom::{
            CustomDevice,
            DeviceAccessMode,
            DeviceSharingMode,
        },

        windows::devices::enumeration::{
            DeviceInformation,
            DeviceInformationCollection,
        },

        windows::storage::streams::{
            DataWriter,
            DataReader,
            IBuffer,
            IOutputStream,
            IInputStream,
            DataReaderLoadOperation,
        },

        windows::win32::hid::{
            HidD_GetPreparsedData,
            HidD_FreePreparsedData,
        },

        windows::win32::file_system::CreateFileW,

        windows::win32::system_services::{
            InitiateSystemShutdownExW,
            SetSuspendState,
            GetCurrentThread,
            CreateServiceW,
            CreateEventW,
            UnregisterWaitEx,
            SetEvent,
            ResetEvent,
        },

        windows::win32::security::{
            StartServiceCtrlDispatcherW,
            RegisterServiceCtrlHandlerExW,
            SetServiceStatus,
            OpenThreadToken,
            LookupPrivilegeValueW,
            ImpersonateSelf,
            RevertToSelf,
            OpenSCManagerW,
            CloseServiceHandle,
            DeleteService,
            OpenServiceW,
            ChangeServiceConfig2W,
        },

        windows::win32::debug::OutputDebugStringW,

        windows::win32::windows_programming::CloseHandle,

        windows::win32::remote_desktop_services::{
            WTSCloseServer,
            WTSEnumerateSessionsExW,
            WTSFreeMemoryExW,
            WTSSendMessageW,
        },

        windows::win32::system_services::{
            NTSTATUS,
            PWSTR,
            HANDLE,
            PWSTR,
            RegisterWaitForSingleObject_dwFlags,
        },

        windows::win32::security::{
            LUID_AND_ATTRIBUTES,
        },

        windows::win32::security::{
            SERVICE_REQUIRED_PRIVILEGES_INFOW,
        },

        windows::win32::system_services::{
            ERROR_SUCCESS,
            ERROR_CALL_NOT_IMPLEMENTED,
            ERROR_BADKEY,
            ERROR_ARENA_TRASHED,
            ERROR_NOT_ALL_ASSIGNED,
            E_UNEXPECTED,
            E_INVALIDARG,
            E_UNEXPECTED,
            OLE_E_CANTCONVERT,
            RPC_E_TIMEOUT,
        },

        windows::win32::windows_and_messaging::{
            PBT_APMRESUMEAUTOMATIC,
        }
    );
}

#![feature(negative_impls)]

mod config;
mod event;
mod logger;
mod self_impersonator;
mod services;
mod sessions;
mod token;

use std::{
    env,
    error::Error,
    ffi::c_void,
    fmt::Display,
    panic::catch_unwind,
    process::abort,
    sync::atomic::{AtomicIsize, AtomicU32, Ordering},
    time::Duration,
};

use humantime::format_duration;
use lazy_static::lazy_static;
use log::{debug, error, info, trace, warn};
use tokio::{
    sync::{watch, Notify},
    time::sleep,
};
use utf16_lit::utf16_null;

use bindings::windows::win32::{
    remote_desktop_services::WTS_CONNECTSTATE_CLASS,
    security::{
        EnumServicesStatus_dwServiceType, RegisterServiceCtrlHandlerExW,
        SERVICE_STATUS_PROCESS_dwCurrentState, SetServiceStatus, StartServiceCtrlDispatcherW,
        SECURITY_IMPERSONATION_LEVEL, SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_TABLE_ENTRYW,
    },
    system_services::{
        CreateServiceW_dwStartType, InitiateSystemShutdownExW, SetSuspendState, BOOL,
        ERROR_ARENA_TRASHED, ERROR_BADKEY, ERROR_CALL_NOT_IMPLEMENTED, ERROR_SUCCESS, PWSTR,
        SERVICE_ERROR,
    },
    windows_and_messaging::PBT_APMRESUMEAUTOMATIC,
};

use config::{HardCodedConfig, RuntimeConfig};
use event::Event;
use logger::LOGGER;
use self_impersonator::SelfImpersonator;
use services::{ScManager, ScManagerAccessRights, ServiceAccessRights};
use sessions::WTSServer;
use token::Token;
use ups::hid_device::HidDevice;
use ups::hid_ups::{HidUps, UpsStatus, UpsStatusFlags, UpsWorkMode};

static SERVICE_HANDLE: AtomicIsize = AtomicIsize::new(0);
static SHUTDOWN: Notify = Notify::const_new();
lazy_static! {
    static ref WAKEUP: Event = Event::new(true, false).unwrap();
}

fn main() -> Result<(), Box<dyn Error>> {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);

    if let Some(argument) = env::args().nth(1) {
        let argument = argument.to_lowercase();
        if argument == "install" {
            return install_service();
        } else if argument == "uninstall" {
            uninstall_service()?;
            return Ok(());
        }
    }

    debug!("Starting service control dispatcher...");
    unsafe {
        let mut name = utf16_null!(HardCodedConfig::SERVICE_NAME);
        let table = [
            SERVICE_TABLE_ENTRYW {
                lp_service_name: PWSTR(name.as_mut_ptr()),
                lp_service_proc: Some(service_main),
            },
            SERVICE_TABLE_ENTRYW {
                lp_service_name: PWSTR::default(),
                lp_service_proc: None,
            },
        ];
        StartServiceCtrlDispatcherW(table.as_ptr()).ok()?;
    }

    Ok(())
}

fn install_service() -> Result<(), Box<dyn Error>> {
    let sc_manager = ScManager::open_local(ScManagerAccessRights::SC_MANAGER_CREATE_SERVICE)?;

    let service = sc_manager.create_local_system_service(
        HardCodedConfig::SERVICE_NAME,
        HardCodedConfig::SERVICE_DISPLAY_NAME,
        EnumServicesStatus_dwServiceType::SERVICE_WIN32_OWN_PROCESS,
        CreateServiceW_dwStartType::SERVICE_AUTO_START,
        SERVICE_ERROR::SERVICE_ERROR_NORMAL,
        env::current_exe().unwrap(),
    )?;

    let set_privilege_result = service.set_required_privileges(&["SeShutdownPrivilege"]);
    if set_privilege_result.is_err() {
        service.delete().unwrap();
        set_privilege_result?;
        unreachable!();
    }

    let config_write_result = RuntimeConfig::default().write();
    if config_write_result.is_err() {
        service.delete().unwrap();
        config_write_result?;
        unreachable!();
    }

    Ok(())
}

fn uninstall_service() -> windows::Result<()> {
    let sc_manager = ScManager::open_local(ScManagerAccessRights::SC_MANAGER_CONNECT)?;

    let service =
        sc_manager.open_service(HardCodedConfig::SERVICE_NAME, ServiceAccessRights::DELETE)?;

    service.delete()?;

    Ok(())
}

extern "system" fn service_main(_dw_num_services_args: u32, _lp_service_arg_vectors: *mut PWSTR) {
    let result = catch_unwind(|| {
        debug!("Registering service control handler...");
        unsafe {
            let handle = RegisterServiceCtrlHandlerExW(
                PWSTR(utf16_null!(HardCodedConfig::SERVICE_NAME).as_mut_ptr()),
                Some(control_handler),
                std::ptr::null_mut(),
            );
            if handle.0 == 0 {
                let error = windows::Error::from(windows::ErrorCode::from_thread());
                error!("RegisterServiceCtrlHandlerExW failed with {:?}", error);
                return;
            }
            SERVICE_HANDLE.store(handle.0, Ordering::SeqCst);
        }

        report_service_status(
            ServiceState::SERVICE_START_PENDING,
            ERROR_SUCCESS,
            HardCodedConfig::MAX_START_TIME_MS,
        );

        run_service();
    });
    if let Err(error) = result {
        error!("ServiceMain panicked: {:?}", error);
        abort();
    }
}

extern "system" fn control_handler(
    dw_control: u32,
    dw_event_type: u32,
    _lp_event_data: *mut c_void,
    _lp_context: *mut c_void,
) -> u32 {
    let result = catch_unwind(|| {
        const SERVICE_CONTROL_STOP: u32 = 0x00000001;
        const SERVICE_CONTROL_INTERROGATE: u32 = 0x00000004;
        const SERVICE_CONTROL_POWEREVENT: u32 = 0x0000000D;

        match dw_control {
            SERVICE_CONTROL_STOP => {
                debug!("SERVICE_CONTROL_STOP");

                report_service_status(
                    ServiceState::SERVICE_STOP_PENDING,
                    ERROR_SUCCESS,
                    HardCodedConfig::MAX_STOP_TIME_MS,
                );

                SHUTDOWN.notify_one();

                ERROR_SUCCESS
            }

            SERVICE_CONTROL_POWEREVENT => {
                if dw_event_type == PBT_APMRESUMEAUTOMATIC {
                    if let Err(error) = WAKEUP.set() {
                        warn!("Signaling a wakeup failed with {:?}", error);
                    }
                }

                ERROR_SUCCESS
            }

            SERVICE_CONTROL_INTERROGATE => ERROR_SUCCESS,

            _ => ERROR_CALL_NOT_IMPLEMENTED,
        }
    });
    match result {
        Ok(status) => status,
        Err(error) => {
            error!("Service control handler panicked: {:?}", error);
            abort();
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn run_service() {
    let config = match RuntimeConfig::read() {
        Ok(config) => config,
        Err(error) => {
            error!("Reading configuration failed with {:?}", error);
            report_service_status(ServiceState::SERVICE_STOPPED, ERROR_BADKEY, 0);
            return;
        }
    };
    debug!("{:?}", config);

    let (tx, rx) = watch::channel(None);

    report_service_status(ServiceState::SERVICE_RUNNING, ERROR_SUCCESS, 0);

    tokio::select! {
        result = ups_query_task(&config, tx) => {
            if let Err(error) = result {
                error!("UPS query failed with {:?}", error);
                report_service_status(ServiceState::SERVICE_STOPPED, ERROR_ARENA_TRASHED, 0);
                return;
            } else {
                unreachable!();
            }
        }
        result = main_loop(&config, rx) => {
            if let Err(error) = result {
                error!("Main loop failed with {:?}", error);
                report_service_status(ServiceState::SERVICE_STOPPED, ERROR_ARENA_TRASHED, 0);
                return;
            } else {
                unreachable!();
            }
        }
        () = SHUTDOWN.notified() => {}
    };

    report_service_status(ServiceState::SERVICE_STOPPED, ERROR_SUCCESS, 0);
}

async fn ups_query_task(
    config: &RuntimeConfig,
    tx: watch::Sender<Option<UpsStatus>>,
) -> windows::Result<()> {
    loop {
        {
            let device = HidDevice::new(
                config.hid_usage_page,
                config.hid_usage_id,
                config.vendor_id,
                config.product_id,
            )
            .await?;
            let ups = HidUps::new(device)?;

            while let Ok(status) = ups.status().await {
                let _ignore = tx.send(Some(status));
                sleep(Duration::from_millis(config.poll_interval_ms.into())).await;
            }
        }

        warn!("UPS query failed");
        sleep(Duration::from_millis(config.poll_failure_timeout_ms.into())).await;
    }
}

async fn main_loop(
    config: &RuntimeConfig,
    rx: watch::Receiver<Option<UpsStatus>>,
) -> Result<(), Box<dyn Error>> {
    loop {
        wait_for_power_loss(rx.clone()).await?;

        {
            let shutdown_timeout = Duration::from_secs(config.shutdown_timeout_s.into());

            send_shutdown_message(shutdown_timeout, config.hibernate);

            tokio::select! {
                () = sleep(shutdown_timeout) => {
                    info!("Timer elapsed, initiating shutdown...");
                    WAKEUP.reset()?;
                    initiate_shutdown(config.hibernate)?;
                }
                result = wait_for_low_battery(rx.clone()) => {
                    result?;
                    warn!("Low battery detected, shutting down ahead of time...");
                    WAKEUP.reset()?;
                    initiate_shutdown(config.hibernate)?;
                }
                result = wait_for_power_recovery(rx.clone()) => {
                    result?;
                    info!("Power restored");
                    continue;
                }
            };
        }

        // Shutdown/hibernation initiated.

        {
            tokio::select! {
                () = WAKEUP.signaled()? => {
                    info!("System woke up");
                }
                result = wait_for_power_recovery(rx.clone()) => {
                    result?;
                    info!("Power restored");
                }
            }
        }
    }
}

async fn wait_for_power_loss(rx: watch::Receiver<Option<UpsStatus>>) -> Result<(), Box<dyn Error>> {
    wait_for_ups_status(rx, |status| match status.work_mode() {
        UpsWorkMode::Battery | UpsWorkMode::BatteryTest => {
            warn!("Power loss detected");
            true
        }
        UpsWorkMode::Fault => {
            warn!("UPS fault detected");
            true
        }
        _ => false,
    })
    .await
}

async fn wait_for_power_recovery(
    rx: watch::Receiver<Option<UpsStatus>>,
) -> Result<(), Box<dyn Error>> {
    wait_for_ups_status(rx, |status| status.work_mode() == UpsWorkMode::Line).await
}

async fn wait_for_low_battery(
    rx: watch::Receiver<Option<UpsStatus>>,
) -> Result<(), Box<dyn Error>> {
    wait_for_ups_status(rx, |status| {
        status.flags().contains(UpsStatusFlags::BATTERY_LOW)
    })
    .await
}

async fn wait_for_ups_status<F>(
    mut rx: watch::Receiver<Option<UpsStatus>>,
    mut predicate: F,
) -> Result<(), Box<dyn Error>>
where
    F: FnMut(&UpsStatus) -> bool,
{
    loop {
        rx.changed().await?;
        if let Some(status) = &*rx.borrow() {
            if predicate(status) {
                return Ok(());
            }
        }
    }
}

fn initiate_shutdown(hibernate: bool) -> windows::Result<()> {
    let _impersonator =
        SelfImpersonator::impersonate(SECURITY_IMPERSONATION_LEVEL::SecurityImpersonation)?;

    const TOKEN_ADJUST_PRIVILEGES: u32 = 0x00000020;
    let thread_token = Token::open_thread_token(TOKEN_ADJUST_PRIVILEGES, true)?;

    const SE_SHUTDOWN_NAME: &str = "SeShutdownPrivilege";
    thread_token.enable_privilege(SE_SHUTDOWN_NAME)?;

    if hibernate {
        info!("Hibernating...");
        let result = unsafe { SetSuspendState(1, 0, 1) };
        let result = BOOL::from(result != 0);
        result.ok()?;
    } else {
        info!("Shutting down...");
        const SHTDN_REASON_MAJOR_POWER: u32 = 0x00060000;
        const SHTDN_REASON_MINOR_ENVIRONMENT: u32 = 0x0000000C;
        unsafe {
            InitiateSystemShutdownExW(
                PWSTR::default(),
                PWSTR::default(),
                0,
                false,
                false,
                SHTDN_REASON_MAJOR_POWER | SHTDN_REASON_MINOR_ENVIRONMENT,
            )
            .ok()?;
        };
    };

    Ok(())
}

fn send_shutdown_message(time: Duration, hibernate: bool) {
    let formatted_duration = format_duration(time);

    let message = format!(
        "Power loss detected.\n\nUnless power is restored within the next {}, the system will {}.",
        formatted_duration,
        if hibernate { "hibernate" } else { "shut down" }
    );

    warn!("System going down in {}", formatted_duration);
    notify_active_users(HardCodedConfig::SERVICE_DISPLAY_NAME, message);
}

fn notify_active_users(title: impl AsRef<str>, message: impl AsRef<str>) {
    let server = WTSServer::open_local();
    if let Ok(sessions) = server.sessions() {
        sessions
            .iter()
            .filter(|session| session.connection_state() == WTS_CONNECTSTATE_CLASS::WTSActive)
            .filter(|session| session.is_local_session())
            .for_each(|session| {
                trace!(
                    "Notifying session {} of imminent shutdown",
                    session.session_id()
                );

                const MB_OK: u32 = 0x00000000;
                const MB_ICONWARNING: u32 = 0x00000030;
                if let Err(error) = server.send_message(
                    session.session_id(),
                    title.as_ref(),
                    message.as_ref(),
                    MB_OK | MB_ICONWARNING,
                ) {
                    warn!(
                        "Session {} notification failed with {:?}",
                        session.session_id(),
                        error
                    );
                }
            });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ServiceState(SERVICE_STATUS_PROCESS_dwCurrentState);

impl ServiceState {
    const SERVICE_STOPPED: Self = Self(SERVICE_STATUS_PROCESS_dwCurrentState::SERVICE_STOPPED);
    const SERVICE_START_PENDING: Self =
        Self(SERVICE_STATUS_PROCESS_dwCurrentState::SERVICE_START_PENDING);
    const SERVICE_STOP_PENDING: Self =
        Self(SERVICE_STATUS_PROCESS_dwCurrentState::SERVICE_STOP_PENDING);
    const SERVICE_RUNNING: Self = Self(SERVICE_STATUS_PROCESS_dwCurrentState::SERVICE_RUNNING);
}

impl Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            &Self::SERVICE_STOPPED => write!(f, "SERVICE_STOPPED"),
            &Self::SERVICE_START_PENDING => write!(f, "SERVICE_START_PENDING"),
            &Self::SERVICE_STOP_PENDING => write!(f, "SERVICE_STOP_PENDING"),
            &Self::SERVICE_RUNNING => write!(f, "SERVICE_RUNNING"),
            _ => write!(f, "{:?}", self),
        }
    }
}

fn report_service_status(current_state: ServiceState, win32_exit_code: u32, wait_hint_ms: u32) {
    debug!("{}, {}, {}", current_state, win32_exit_code, wait_hint_ms);

    const SERVICE_ACCEPT_STOP: u32 = 0x00000001;
    const SERVICE_ACCEPT_POWEREVENT: u32 = 0x00000040;

    let mut status = SERVICE_STATUS {
        dw_service_type: bindings::windows::win32::security::EnumServicesStatus_dwServiceType::SERVICE_WIN32_OWN_PROCESS,
        dw_current_state: current_state.0,
        dw_win32_exit_code: win32_exit_code,
        dw_wait_hint: wait_hint_ms,
        ..Default::default()
    };

    status.dw_controls_accepted = if current_state == ServiceState::SERVICE_START_PENDING {
        0
    } else {
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_POWEREVENT
    };

    static CHECKPOINT: AtomicU32 = AtomicU32::new(1);
    status.dw_check_point = match current_state {
        ServiceState::SERVICE_RUNNING | ServiceState::SERVICE_STOPPED => 0,
        _ => CHECKPOINT.fetch_add(1, Ordering::SeqCst),
    };

    trace!("{:?}", status);

    unsafe {
        SetServiceStatus(
            SERVICE_STATUS_HANDLE(SERVICE_HANDLE.load(Ordering::SeqCst)),
            &mut status,
        );
    }
}

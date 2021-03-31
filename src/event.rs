use std::{
    ffi::c_void,
    future::Future,
    marker::PhantomData,
    panic::catch_unwind,
    pin::Pin,
    process::abort,
    sync::Mutex,
    task::{Context, Poll, Waker},
};

use log::error;
use static_assertions::assert_impl_all;
use windows::{Error, ErrorCode};

use bindings::windows::win32::{
    system_services::{
        CreateEventW, RegisterWaitForSingleObject_dwFlags, ResetEvent, SetEvent, UnregisterWaitEx,
        BOOL, HANDLE, PWSTR,
    },
    windows_programming::CloseHandle,
};

pub struct Event {
    handle: HANDLE,
}

impl Event {
    pub fn new(manual_reset: bool, signaled: bool) -> windows::Result<Self> {
        let handle = unsafe {
            CreateEventW(
                std::ptr::null_mut(),
                manual_reset,
                signaled,
                PWSTR::default(),
            )
        };
        if handle == HANDLE(0) {
            return Err(Error::from(ErrorCode::from_thread()));
        }
        Ok(Self { handle })
    }

    pub fn set(&self) -> windows::Result<()> {
        unsafe { SetEvent(self.handle).ok() }
    }

    pub fn reset(&self) -> windows::Result<()> {
        unsafe { ResetEvent(self.handle).ok() }
    }

    pub fn signaled(&self) -> windows::Result<Signaled> {
        Signaled::new(self)
    }

    pub fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle).expect("CloseHandle failed");
        }
    }
}

unsafe impl Send for Event {}
unsafe impl Sync for Event {}

pub struct Signaled<'a> {
    wait_handle: HANDLE,
    shared_state: *const Mutex<SharedState>,
    _event: PhantomData<&'a Event>,
}

#[cfg(test)]
static SHARED_STATE_DROP_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

struct SharedState {
    signaled: bool,
    waker: Option<Waker>,
}

#[cfg(test)]
impl Drop for SharedState {
    fn drop(&mut self) {
        SHARED_STATE_DROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl<'a> Signaled<'a> {
    fn new(event: &'a Event) -> windows::Result<Self> {
        let shared_state = SharedState {
            signaled: false,
            waker: None,
        };
        let shared_state = Mutex::new(shared_state);
        let shared_state = Box::new(shared_state);

        let (wait_handle, shared_state) = Self::register_wait(event, shared_state)?;

        let result = Self {
            wait_handle,
            shared_state,
            _event: PhantomData,
        };
        Ok(result)
    }

    fn register_wait(
        event: &Event,
        shared_state: Box<Mutex<SharedState>>,
    ) -> windows::Result<(HANDLE, *const Mutex<SharedState>)> {
        const INFINITE: u32 = u32::MAX;

        assert_impl_all!(Mutex<SharedState>: Sync);

        unsafe {
            let shared_state_raw_ptr = Box::into_raw(shared_state) as *const Mutex<SharedState>;
            let mut wait_handle = Default::default();
            let success = RegisterWaitForSingleObject(
                &mut wait_handle,
                event.raw_handle(),
                Some(Self::wait_callback),
                shared_state_raw_ptr as _,
                INFINITE,
                RegisterWaitForSingleObject_dwFlags::WT_EXECUTEONLYONCE,
            );
            if !success.as_bool() {
                let error_code = ErrorCode::from_thread();
                Self::drop_shared_state(shared_state_raw_ptr);
                return Err(windows::Error::from(error_code));
            }
            Ok((wait_handle, shared_state_raw_ptr))
        }
    }

    unsafe fn drop_shared_state(shared_state: *const Mutex<SharedState>) {
        drop(Box::from_raw(shared_state as *mut Mutex<SharedState>));
    }

    extern "system" fn wait_callback(lp_parameter: *mut c_void, timer_or_wait_fired: u8) {
        let result = catch_unwind(|| {
            let shared_state = lp_parameter as *const Mutex<SharedState>;
            let shared_state = unsafe { shared_state.as_ref().unwrap() };
            let mut shared_state = shared_state.lock().unwrap();

            let timed_out = timer_or_wait_fired != 0;
            assert!(!timed_out); // Can't time out as we specify INFINITE

            shared_state.signaled = true;
            if let Some(waker) = shared_state.waker.take() {
                waker.wake();
            };
        });
        if let Err(error) = result {
            error!("Wait callback panicked: {:?}", error);
            abort();
        }
    }
}

impl<'a> Future for Signaled<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let shared_state = unsafe { self.shared_state.as_ref().unwrap() };
        let mut shared_state = shared_state.lock().unwrap();

        if shared_state.signaled {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<'a> Drop for Signaled<'a> {
    fn drop(&mut self) {
        // See: https://doc.rust-lang.org/std/pin/index.html#drop-implementation
        inner_drop(Pin::new(self));
        fn inner_drop<'a>(this: Pin<&mut Signaled<'a>>) {
            unsafe {
                // Specifying INVALID_HANDLE_VALUE so that the call waits for all callbacks
                // to return.
                const INVALID_HANDLE_VALUE: HANDLE = HANDLE(-1);
                assert_ne!(this.wait_handle, HANDLE(0));
                UnregisterWaitEx(this.wait_handle, INVALID_HANDLE_VALUE)
                    .expect("UnregisterWaitEx failed");
                Signaled::drop_shared_state(this.shared_state);
            }
        }
    }
}

type WAITORTIMERCALLBACK = extern "system" fn(lp_parameter: *mut c_void, timer_or_wait_fired: u8);

extern "system" {
    #[link(name = "kernel32")]
    fn RegisterWaitForSingleObject(
        ph_new_wait_object: *mut HANDLE,
        h_object: HANDLE,
        callback: Option<WAITORTIMERCALLBACK>,
        context: *mut c_void,
        dw_milliseconds: u32,
        dw_flags: RegisterWaitForSingleObject_dwFlags,
    ) -> BOOL;
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    #[test]
    fn manual_event_can_be_created() {
        Event::new(true, false).unwrap();
    }

    #[test]
    fn auto_event_can_be_created() {
        Event::new(false, false).unwrap();
    }

    #[tokio::test]
    async fn manual_event_can_be_awaited() {
        let event = Event::new(true, false).unwrap();
        event.set().unwrap();
        event.signaled().unwrap().await;
    }

    #[tokio::test]
    async fn auto_event_can_be_awaited() {
        let event = Event::new(false, false).unwrap();
        event.set().unwrap();
        event.signaled().unwrap().await;
    }

    #[tokio::test]
    async fn manual_event_can_be_awaited_twice() {
        let event = Event::new(true, false).unwrap();
        event.set().unwrap();
        event.signaled().unwrap().await;
        event.signaled().unwrap().await;
    }

    #[test]
    fn manual_event_future_can_be_dropped_without_awaiting() {
        let event = Event::new(true, false).unwrap();
        let _future = event.signaled().unwrap();
    }

    #[test]
    fn auto_event_future_can_be_dropped_without_awaiting() {
        let event = Event::new(false, false).unwrap();
        let _future = event.signaled().unwrap();
    }

    #[test]
    fn manual_event_future_doesnt_leak() {
        SHARED_STATE_DROP_COUNT.store(0, Ordering::SeqCst);

        let event = Event::new(true, false).unwrap();
        let future = event.signaled().unwrap();
        drop(future);

        assert_eq!(SHARED_STATE_DROP_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn auto_event_future_doesnt_leak() {
        SHARED_STATE_DROP_COUNT.store(0, Ordering::SeqCst);

        let event = Event::new(false, false).unwrap();
        let future = event.signaled().unwrap();
        drop(future);

        assert_eq!(SHARED_STATE_DROP_COUNT.load(Ordering::SeqCst), 1);
    }
}

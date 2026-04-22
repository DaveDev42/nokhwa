//! Backend-level hotplug for the `MediaFoundation` backend.
//!
//! Event-driven implementation backed by Win32's
//! `RegisterDeviceNotificationW(KSCATEGORY_VIDEO_CAMERA)` plumbing. A
//! dedicated worker thread owns a hidden message-only window and
//! pumps `WM_DEVICECHANGE` notifications (`DBT_DEVICEARRIVAL` /
//! `DBT_DEVICEREMOVECOMPLETE`). Each notification carries the device
//! interface's symbolic-link path; we re-enumerate via `wmf::query()`
//! and diff against a cached snapshot keyed on that symbolic link to
//! produce `HotplugEvent::Connected` / `Disconnected`.
//!
//! Why event-driven over polling: zero wake-ups in the steady state,
//! immediate notification instead of up-to-500ms latency. The extra
//! complexity (~250 LOC of `unsafe` Win32 glue) is worth it for
//! battery-powered Windows hosts where the 2×/sec poll thread is a
//! measurable power cost.

#[cfg(all(target_os = "windows", not(feature = "docs-only")))]
mod real {
    use crate::wmf::query;
    use nokhwa_core::{
        error::NokhwaError,
        traits::{HotplugEvent, HotplugEventPoll, HotplugSource},
        types::{ApiBackend, CameraInfo},
    };
    use std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver, Sender},
            Arc,
        },
        thread::{self, JoinHandle},
        time::Duration,
    };
    use windows::core::GUID;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        GetWindowLongPtrW, PostThreadMessageW, RegisterClassExW, RegisterDeviceNotificationW,
        SetWindowLongPtrW, TranslateMessage, UnregisterDeviceNotification, CW_USEDEFAULT,
        DBT_DEVICEARRIVAL, DBT_DEVICEREMOVECOMPLETE, DBT_DEVTYP_DEVICEINTERFACE,
        DEVICE_NOTIFY_WINDOW_HANDLE, DEV_BROADCAST_DEVICEINTERFACE_W, GWLP_USERDATA, HDEVNOTIFY,
        HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DEVICECHANGE, WM_QUIT, WNDCLASSEXW,
        WS_OVERLAPPED,
    };

    /// `KSCATEGORY_VIDEO_CAMERA` — the device interface class GUID we
    /// register for notifications on. Matches the filter Media
    /// Foundation uses internally; every UVC / MSMF-visible camera
    /// published on this class surfaces an arrival/removal here.
    const KSCATEGORY_VIDEO_CAMERA: GUID = GUID::from_u128(0xE5323777_F976_4F5B_9B55_B94699C46E44);

    /// Backend-level hotplug source for `MediaFoundation`. Cheap to
    /// construct — the worker thread + hidden window are only
    /// created when
    /// [`take_hotplug_events`](HotplugSource::take_hotplug_events) is
    /// called, and are torn down when the returned poller is dropped.
    #[derive(Default)]
    pub struct MediaFoundationHotplugContext {
        taken: bool,
    }

    impl MediaFoundationHotplugContext {
        #[must_use]
        pub fn new() -> Self {
            Self { taken: false }
        }
    }

    impl HotplugSource for MediaFoundationHotplugContext {
        fn take_hotplug_events(&mut self) -> Result<Box<dyn HotplugEventPoll + Send>, NokhwaError> {
            if self.taken {
                return Err(NokhwaError::UnsupportedOperationError(
                    ApiBackend::MediaFoundation,
                ));
            }
            self.taken = true;
            Ok(Box::new(MsmfHotplugPoll::spawn()?))
        }
    }

    /// Concrete [`HotplugEventPoll`]. Owns the worker thread + a
    /// cross-thread handle (`ThreadId`) used to wake the GetMessage
    /// loop with `WM_QUIT` on drop. The mpsc channel carries events
    /// from the WndProc-synthesised callbacks back to the consumer.
    struct MsmfHotplugPoll {
        rx: Receiver<HotplugEvent>,
        stop: Arc<AtomicBool>,
        worker_thread_id: u32,
        join: Option<JoinHandle<()>>,
    }

    impl MsmfHotplugPoll {
        fn spawn() -> Result<Self, NokhwaError> {
            let (tx, rx) = mpsc::channel();
            let stop = Arc::new(AtomicBool::new(false));
            let (tid_tx, tid_rx) = mpsc::channel::<u32>();
            let stop_thread = Arc::clone(&stop);
            let join = thread::Builder::new()
                .name("nokhwa-msmf-hotplug".to_string())
                .spawn(move || worker_loop(&tx, &stop_thread, &tid_tx))
                .map_err(|e| NokhwaError::general(format!("spawn hotplug thread: {e}")))?;

            // Wait for the worker to publish its Win32 thread id so we
            // can post WM_QUIT to it during Drop.
            let worker_thread_id = tid_rx.recv_timeout(Duration::from_secs(5)).map_err(|e| {
                NokhwaError::general(format!("worker thread id handshake timed out: {e}"))
            })?;

            Ok(Self {
                rx,
                stop,
                worker_thread_id,
                join: Some(join),
            })
        }
    }

    impl HotplugEventPoll for MsmfHotplugPoll {
        fn try_next(&mut self) -> Option<HotplugEvent> {
            self.rx.try_recv().ok()
        }
        fn next_timeout(&mut self, d: Duration) -> Option<HotplugEvent> {
            self.rx.recv_timeout(d).ok()
        }
    }

    impl Drop for MsmfHotplugPoll {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            // Poke the GetMessage loop so it wakes up, checks the
            // stop flag, and exits. WM_QUIT posted to the thread
            // (rather than to a specific HWND) is the canonical way
            // to break a GetMessage pump from outside.
            unsafe {
                let _ = PostThreadMessageW(self.worker_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(h) = self.join.take() {
                let _ = h.join();
            }
        }
    }

    /// Worker thread: create hidden window, register for device
    /// notifications, pump messages. On arrival/removal,
    /// re-`query()` and diff against the cached snapshot.
    #[allow(clippy::too_many_lines)]
    fn worker_loop(tx: &Sender<HotplugEvent>, stop: &Arc<AtomicBool>, tid_tx: &Sender<u32>) {
        // Publish our thread id for Drop to target.
        let thread_id = unsafe { GetCurrentThreadId() };
        let _ = tid_tx.send(thread_id);

        // Seed the cache. We will emit changes relative to this
        // snapshot — consumers see hotplug deltas, not the initial
        // population (they should call `wmf::query()` directly to
        // learn what's already plugged in).
        let mut snapshot: BTreeMap<String, CameraInfo> = take_snapshot();

        // The WndProc writes events into a Box<SharedState> parked in
        // GWLP_USERDATA. It can't capture `snapshot` directly because
        // WndProc is an `extern "system" fn`, so we pass a pointer to
        // shared state through the window.
        let shared = Box::new(SharedState {
            tx: tx.clone(),
            snapshot: std::cell::RefCell::new(std::mem::take(&mut snapshot)),
        });
        let shared_ptr: *mut SharedState = Box::into_raw(shared);

        let hwnd = match create_hidden_window(shared_ptr) {
            Ok(h) => h,
            Err(e) => {
                // Reclaim the Box so it isn't leaked on error path.
                unsafe {
                    drop(Box::from_raw(shared_ptr));
                }
                eprintln!("nokhwa msmf hotplug: create_hidden_window failed: {e}");
                return;
            }
        };

        let notify_handle = match register_device_notifications(hwnd) {
            Ok(h) => h,
            Err(e) => {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                    drop(Box::from_raw(shared_ptr));
                }
                eprintln!("nokhwa msmf hotplug: RegisterDeviceNotificationW failed: {e}");
                return;
            }
        };

        // Pump messages until WM_QUIT arrives from Drop. WM_DEVICECHANGE
        // is the only signal we act on — if some hypothetical driver
        // doesn't emit WM_DEVICECHANGE for its cameras we'd miss the
        // events entirely, but for the Media-Foundation-visible cameras
        // in the `KSCATEGORY_VIDEO_CAMERA` filter WM_DEVICECHANGE is
        // the documented contract.
        loop {
            let mut msg = MSG::default();
            let rv = unsafe { GetMessageW(&mut msg, Some(hwnd), 0, 0) };
            if rv.0 == 0 || rv.0 == -1 {
                // 0 = WM_QUIT received, -1 = error. Either way, exit.
                break;
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if stop.load(Ordering::Acquire) {
                break;
            }
        }

        // Cleanup.
        unsafe {
            let _ = UnregisterDeviceNotification(notify_handle);
            let _ = DestroyWindow(hwnd);
            drop(Box::from_raw(shared_ptr));
        }
    }

    /// State shared between the worker thread and the WndProc
    /// callback. The WndProc reads `tx` + `snapshot` through the
    /// raw pointer stashed in the window's GWLP_USERDATA slot.
    struct SharedState {
        tx: Sender<HotplugEvent>,
        snapshot: std::cell::RefCell<BTreeMap<String, CameraInfo>>,
    }

    fn create_hidden_window(shared: *mut SharedState) -> Result<HWND, String> {
        let class_name: Vec<u16> = "nokhwa_msmf_hotplug\0".encode_utf16().collect();
        let title: Vec<u16> = "nokhwa_msmf_hotplug\0".encode_utf16().collect();

        unsafe {
            let hinstance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wnd_proc),
                hInstance: hinstance.into(),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            // RegisterClassExW may fail if the class is already
            // registered by a previous MsmfHotplugPoll instance; that's
            // fine, CreateWindowExW below will still find the class.
            let _ = RegisterClassExW(&wc);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                WINDOW_STYLE(WS_OVERLAPPED.0),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                Some(HWND_MESSAGE), // message-only window — not shown in taskbar
                None,
                Some(hinstance.into()),
                None,
            )
            .map_err(|e| e.to_string())?;
            // Stash the shared-state pointer in GWLP_USERDATA so the
            // static WndProc can fetch it on each WM_DEVICECHANGE.
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, shared as isize);
            Ok(hwnd)
        }
    }

    fn register_device_notifications(hwnd: HWND) -> Result<HDEVNOTIFY, String> {
        unsafe {
            let mut filter = DEV_BROADCAST_DEVICEINTERFACE_W {
                dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
                dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
                dbcc_classguid: KSCATEGORY_VIDEO_CAMERA,
                ..Default::default()
            };
            RegisterDeviceNotificationW(
                hwnd.into(),
                std::ptr::from_mut::<DEV_BROADCAST_DEVICEINTERFACE_W>(&mut filter).cast(),
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
            .map_err(|e| e.to_string())
        }
    }

    /// Static WndProc. Reads the `SharedState` pointer back from
    /// GWLP_USERDATA, then on each `WM_DEVICECHANGE` (or our
    /// `WM_HEARTBEAT`) takes a fresh snapshot and emits deltas.
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_DEVICECHANGE {
            let shared_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut SharedState;
            if !shared_ptr.is_null() {
                // Only reconcile on arrival / removal — other
                // WM_DEVICECHANGE events (DBT_QUERYREMOVE, etc.) are
                // nice-to-haves but not required for the snapshot
                // model.
                let event = wparam.0 as u32;
                let should_reconcile =
                    event == DBT_DEVICEARRIVAL || event == DBT_DEVICEREMOVECOMPLETE;
                if should_reconcile {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let shared: &SharedState = unsafe { &*shared_ptr };
                        reconcile_and_emit(shared);
                    }));
                }
                // Suppress unused_variable; we don't actually inspect
                // the broadcast struct for the symbolic link because
                // re-enumerating is cheap and reliable.
                let _ = lparam;
            }
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// Re-snapshot + diff against the cached one. Emits
    /// `Connected` for newcomers and `Disconnected` for removals,
    /// then swaps the cache. Keyed on `CameraInfo.misc` (the MSMF
    /// symbolic link) — the same key the polling impl used.
    fn reconcile_and_emit(shared: &SharedState) {
        let current = take_snapshot();
        let mut previous = shared.snapshot.borrow_mut();

        for (key, info) in &current {
            if !previous.contains_key(key)
                && shared
                    .tx
                    .send(HotplugEvent::Connected(info.clone()))
                    .is_err()
            {
                return; // consumer dropped the poller
            }
        }
        for (key, info) in previous.iter() {
            if !current.contains_key(key)
                && shared
                    .tx
                    .send(HotplugEvent::Disconnected(info.clone()))
                    .is_err()
            {
                return;
            }
        }
        *previous = current;
    }

    /// One `MediaFoundation` enumeration pass, indexed by the MSMF
    /// symbolic link that the crate already stores in
    /// `CameraInfo.misc`. Swallows errors — transient enumeration
    /// failures should not tear the worker thread down.
    fn take_snapshot() -> BTreeMap<String, CameraInfo> {
        match query() {
            Ok(list) => list.into_iter().map(|ci| (ci.misc(), ci)).collect(),
            Err(_) => BTreeMap::new(),
        }
    }
}

#[cfg(any(not(target_os = "windows"), feature = "docs-only"))]
mod real {
    use nokhwa_core::{
        error::NokhwaError,
        traits::{HotplugEventPoll, HotplugSource},
        types::ApiBackend,
    };

    /// Non-Windows stub for [`MediaFoundationHotplugContext`]. Every
    /// method errors with
    /// [`NokhwaError::UnsupportedOperationError`].
    #[derive(Default)]
    pub struct MediaFoundationHotplugContext;

    impl MediaFoundationHotplugContext {
        #[must_use]
        pub fn new() -> Self {
            Self
        }
    }

    impl HotplugSource for MediaFoundationHotplugContext {
        fn take_hotplug_events(&mut self) -> Result<Box<dyn HotplugEventPoll + Send>, NokhwaError> {
            Err(NokhwaError::UnsupportedOperationError(
                ApiBackend::MediaFoundation,
            ))
        }
    }
}

pub use real::MediaFoundationHotplugContext;

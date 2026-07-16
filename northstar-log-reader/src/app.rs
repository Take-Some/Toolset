#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(not(windows))]
pub fn run(_initial_url: &str) -> Result<(), String> {
    Err("native LogReader app is currently implemented for Windows only".to_owned())
}

#[cfg(windows)]
mod win32_app {
    use crate::event::NormalizedLogEvent;
    use crate::http;
    use std::ffi::c_void;
    use std::fs;
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::thread;
    use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, DrawTextW, FillRect, GetStockObject, RedrawWindow,
        SetBkMode, SetTextColor, DEFAULT_GUI_FONT, DT_END_ELLIPSIS, DT_LEFT,
        DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, HDC, RDW_INVALIDATE, RDW_NOCHILDREN,
        RDW_NOERASE, TRANSPARENT,
    };
    use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, MEASUREITEMSTRUCT};
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_FILEMUSTEXIST,
        OFN_HIDEREADONLY, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const WM_LOG_EVENT: u32 = WM_APP + 1;
    const WM_STREAM_ENDED: u32 = WM_APP + 2;
    const TIMER_CONNECT_SPINNER: usize = 201;
    const TIMER_LOG_DRAIN: usize = 202;
    const SPINNER_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
    const ID_URL: i32 = 101;
    const ID_CONNECT: i32 = 102;
    const ID_LOGS: i32 = 103;
    const ID_LEVEL: i32 = 104;
    const ID_SEARCH: i32 = 105;
    const ID_CLEAR: i32 = 106;
    const ID_STATUS: i32 = 107;
    const ID_FILE_OPEN: i32 = 201;
    const ID_FILE_SAVE: i32 = 202;
    const ID_FILE_EXIT: i32 = 203;
    const ID_ABOUT: i32 = 204;
    const MF_STRING_LOCAL: u32 = 0x0000;
    const MF_POPUP_LOCAL: u32 = 0x0010;
    const MF_SEPARATOR_LOCAL: u32 = 0x0800;
    const CB_ADDSTRING: u32 = 0x0143;
    const CB_GETCURSEL: u32 = 0x0147;
    const CB_SETCURSEL: u32 = 0x014E;
    const LB_ADDSTRING: u32 = 0x0180;
    const LB_RESETCONTENT: u32 = 0x0184;
    const LB_GETCOUNT: u32 = 0x018B;
    const LB_GETTOPINDEX: u32 = 0x018E;
    const LB_SETTOPINDEX: u32 = 0x0197;
    const LBS_OWNERDRAWFIXED: u32 = 0x0010;
    const LBS_HASSTRINGS: u32 = 0x0040;
    const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;
    const ODS_SELECTED_LOCAL: u32 = 0x0001;
    const WM_SETREDRAW_LOCAL: u32 = 0x000B;
    const MAX_UI_BATCH_EVENTS: usize = 96;
    const MAX_PENDING_LOG_EVENTS: usize = 4096;
    const MAX_RETAINED_LOG_EVENTS: usize = 2000;
    const LOG_DRAIN_INTERVAL_MS: u32 = 33;

    static PENDING_LOG_EVENTS: OnceLock<Mutex<Vec<NormalizedLogEvent>>> = OnceLock::new();
    static PENDING_LOG_POSTED: AtomicBool = AtomicBool::new(false);

    #[inline]
    fn style(value: i32) -> u32 {
        value as u32
    }

    #[derive(Clone)]
    struct AppStateHandle(Arc<Mutex<AppState>>);

    struct AppState {
        hwnd: HWND,
        url: HWND,
        connect: HWND,
        logs: HWND,
        level: HWND,
        search: HWND,
        clear: HWND,
        status: HWND,
        connected: bool,
        connecting: bool,
        spinner_index: usize,
        events: Vec<NormalizedLogEvent>,
        visible_indices: Vec<usize>,
        stop_tx: Option<Sender<()>>,
    }

    impl AppState {
        fn new() -> Self {
            Self {
                hwnd: std::ptr::null_mut(),
                url: std::ptr::null_mut(),
                connect: std::ptr::null_mut(),
                logs: std::ptr::null_mut(),
                level: std::ptr::null_mut(),
                search: std::ptr::null_mut(),
                clear: std::ptr::null_mut(),
                status: std::ptr::null_mut(),
                connected: false,
                connecting: false,
                spinner_index: 0,
                events: Vec::new(),
                visible_indices: Vec::new(),
                stop_tx: None,
            }
        }
    }

    pub fn run(initial_url: &str) -> Result<(), String> {
        // SAFETY: Win32 GUI initialization is process-local; all HWNDs are created and used on this UI thread.
        unsafe { run_win32(initial_url) }
    }

    unsafe fn run_win32(initial_url: &str) -> Result<(), String> {
        let hinstance = GetModuleHandleW(null());
        if hinstance.is_null() {
            return Err("GetModuleHandleW failed".to_owned());
        }

        let class_name = wstr("NorthStarLogReaderWindow");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: LoadIconW(null_mut(), IDI_APPLICATION),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };

        if RegisterClassW(&wc) == 0 {
            return Err("RegisterClassW failed".to_owned());
        }

        let state = AppStateHandle(Arc::new(Mutex::new(AppState::new())));
        let state_ptr = Box::into_raw(Box::new(state.clone()));
        let title = wstr("North Star LIVE LogReader");

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1180,
            760,
            null_mut(),
            null_mut(),
            hinstance,
            state_ptr as *const c_void,
        );

        if hwnd.is_null() {
            let _ = Box::from_raw(state_ptr);
            return Err("CreateWindowExW failed".to_owned());
        }

        if !initial_url.is_empty() {
            if let Ok(guard) = state.0.lock() {
                set_window_text(guard.url, initial_url);
            }
        }

        let mut msg = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_NCCREATE => {
                let createstruct = lparam as *const CREATESTRUCTW;
                if !createstruct.is_null() {
                    let state_ptr = (*createstruct).lpCreateParams as *mut AppStateHandle;
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CREATE => {
                create_menu_bar(hwnd);
                create_controls(hwnd);
                0
            }
            WM_SIZE => {
                layout(hwnd);
                0
            }
            WM_MEASUREITEM => {
                let measure = lparam as *mut MEASUREITEMSTRUCT;
                if !measure.is_null() && (*measure).CtlID == ID_LOGS as u32 {
                    (*measure).itemHeight = 24;
                    return 1;
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DRAWITEM => draw_log_item(hwnd, lparam as *const DRAWITEMSTRUCT),
            WM_COMMAND => {
                let id = (wparam & 0xffff) as i32;
                match id {
                    ID_CONNECT => toggle_connect(hwnd),
                    ID_CLEAR => clear_history(hwnd),
                    ID_FILE_OPEN => open_log_file(hwnd),
                    ID_FILE_SAVE => save_visible_logs(hwnd),
                    ID_FILE_EXIT => { DestroyWindow(hwnd); },
                    ID_ABOUT => show_about(hwnd),
                    ID_LEVEL | ID_SEARCH => rebuild_log_list(hwnd),
                    _ => {}
                }
                0
            }
            WM_LOG_EVENT => {
                drain_pending_log_events(hwnd);
                0
            }
            WM_TIMER => {
                if wparam == TIMER_CONNECT_SPINNER {
                    tick_connect_spinner(hwnd);
                    return 0;
                }
                if wparam == TIMER_LOG_DRAIN {
                    drain_pending_log_events(hwnd);
                    return 0;
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_STREAM_ENDED => {
                stream_ended(hwnd, wparam != 0);
                0
            }
            WM_DESTROY => {
                stop_stream(hwnd);
                if let Some(ptr) = raw_state_ptr(hwnd) {
                    let _ = Box::from_raw(ptr);
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn create_menu_bar(hwnd: HWND) {
        let menu = CreateMenu();
        let file_menu = CreateMenu();

        let open = wstr("Open...");
        let save = wstr("Save...");
        let exit = wstr("Exit");
        let file = wstr("File");
        let about = wstr("About");

        AppendMenuW(file_menu, MF_STRING_LOCAL, ID_FILE_OPEN as usize, open.as_ptr());
        AppendMenuW(file_menu, MF_STRING_LOCAL, ID_FILE_SAVE as usize, save.as_ptr());
        AppendMenuW(file_menu, MF_SEPARATOR_LOCAL, 0, null());
        AppendMenuW(file_menu, MF_STRING_LOCAL, ID_FILE_EXIT as usize, exit.as_ptr());
        AppendMenuW(menu, MF_POPUP_LOCAL, file_menu as usize, file.as_ptr());
        AppendMenuW(menu, MF_STRING_LOCAL, ID_ABOUT as usize, about.as_ptr());
        SetMenu(hwnd, menu);
    }

    unsafe fn create_controls(hwnd: HWND) {
        let Some(state) = state(hwnd) else { return; };
        let hinstance = GetModuleHandleW(null());
        let font = GetStockObject(DEFAULT_GUI_FONT);

        let url = child(hwnd, "EDIT", "", WS_BORDER | WS_TABSTOP | style(ES_AUTOHSCROLL), ID_URL, hinstance);
        let connect = child(hwnd, "BUTTON", "Connect", WS_TABSTOP | style(BS_PUSHBUTTON), ID_CONNECT, hinstance);
        let logs = child(
            hwnd,
            "LISTBOX",
            "",
            WS_BORDER | WS_VSCROLL | WS_TABSTOP | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT,
            ID_LOGS,
            hinstance,
        );
        let level = child(hwnd, "COMBOBOX", "", WS_BORDER | WS_TABSTOP | style(CBS_DROPDOWNLIST), ID_LEVEL, hinstance);
        let search = child(hwnd, "EDIT", "", WS_BORDER | WS_TABSTOP | style(ES_AUTOHSCROLL), ID_SEARCH, hinstance);
        let clear = child(hwnd, "BUTTON", "Clear history logs", WS_TABSTOP | style(BS_PUSHBUTTON), ID_CLEAR, hinstance);
        let status = child(hwnd, "STATIC", "idle", 0, ID_STATUS, hinstance);

        for handle in [url, connect, logs, level, search, clear, status] {
            SendMessageW(handle, WM_SETFONT, font as WPARAM, 1);
        }

        for item in ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"] {
            let text = wstr(item);
            SendMessageW(level, CB_ADDSTRING, 0, text.as_ptr() as LPARAM);
        }
        SendMessageW(level, CB_SETCURSEL, 2, 0);
        set_window_text(search, "");

        if let Ok(mut guard) = state.0.lock() {
            guard.hwnd = hwnd;
            guard.url = url;
            guard.connect = connect;
            guard.logs = logs;
            guard.level = level;
            guard.search = search;
            guard.clear = clear;
            guard.status = status;
        }

        layout(hwnd);
    }

    unsafe fn layout(hwnd: HWND) {
        let Some(state) = state(hwnd) else { return; };
        let mut rect = std::mem::zeroed::<RECT>();
        if GetClientRect(hwnd, &mut rect) == 0 {
            return;
        }
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let margin = 12;
        let top_h = 34;
        let bottom_h = 38;
        let gap = 8;
        let button_w = 104;
        let level_w = 150;
        let clear_w = 180;
        let status_w = 170;

        if let Ok(guard) = state.0.lock() {
            MoveWindow(guard.url, margin, margin, width - margin * 2 - button_w - status_w - gap * 2, top_h, 1);
            MoveWindow(guard.connect, width - margin - button_w - status_w - gap, margin, button_w, top_h, 1);
            MoveWindow(guard.status, width - margin - status_w, margin + 8, status_w, 20, 1);

            let log_y = margin + top_h + gap;
            let log_h = height - top_h - bottom_h - margin * 3 - gap * 2;
            MoveWindow(guard.logs, margin, log_y, width - margin * 2, log_h.max(80), 1);

            let bottom_y = height - margin - bottom_h;
            MoveWindow(guard.level, margin, bottom_y, level_w, 160, 1);
            MoveWindow(guard.search, margin + level_w + gap, bottom_y, width - margin * 2 - level_w - clear_w - gap * 2, bottom_h, 1);
            MoveWindow(guard.clear, width - margin - clear_w, bottom_y, clear_w, bottom_h, 1);
        };
    }

    unsafe fn toggle_connect(hwnd: HWND) {
        let Some(state) = state(hwnd) else { return; };
        let connected = state.0.lock().map(|guard| guard.connected).unwrap_or(false);
        if connected {
            stop_stream(hwnd);
            return;
        }

        let url = state.0.lock().map(|guard| get_window_text(guard.url)).unwrap_or_default();
        if url.trim().is_empty() {
            if let Ok(guard) = state.0.lock() {
                set_window_text(guard.status, "enter URL");
            }
            return;
        }
        if !url.starts_with("http://") {
            if let Ok(guard) = state.0.lock() {
                set_window_text(guard.status, "http:// only");
            }
            return;
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        if let Ok(mut guard) = state.0.lock() {
            guard.connected = true;
            guard.connecting = true;
            guard.spinner_index = 0;
            guard.stop_tx = Some(stop_tx);
            set_window_text(guard.connect, "Disconnect");
            set_window_text(guard.status, "LIVE connecting |");
            SetTimer(hwnd, TIMER_CONNECT_SPINNER, 120, None);
        }

        let target_hwnd = hwnd as isize;
        thread::spawn(move || stream_worker(target_hwnd, url, stop_rx));
    }

    unsafe fn stop_stream(hwnd: HWND) {
        if let Some(state) = state(hwnd) {
            if let Ok(mut guard) = state.0.lock() {
                if let Some(stop_tx) = guard.stop_tx.take() {
                    let _ = stop_tx.send(());
                }
                guard.connected = false;
                guard.connecting = false;
                KillTimer(hwnd, TIMER_CONNECT_SPINNER);
                KillTimer(hwnd, TIMER_LOG_DRAIN);
                PENDING_LOG_POSTED.store(false, Ordering::Release);
                if let Ok(mut queue) = pending_log_events().lock() {
                    queue.clear();
                }
                set_window_text(guard.connect, "Connect");
                set_window_text(guard.status, "disconnected");
            }
        }
    }

    fn stream_worker(hwnd_value: isize, url: String, stop_rx: Receiver<()>) {
        let hwnd = hwnd_value as HWND;
        let result = http::stream_http_events(&url, None, |event| {
            if stop_rx.try_recv().is_ok() {
                return Err("stream stopped".to_owned());
            }
            queue_log_event(hwnd, event.clone());
            Ok(())
        });

        let failed = result.is_err();
        // SAFETY: posts a completion notification to the UI thread; no borrowed data crosses threads.
        unsafe { PostMessageW(hwnd, WM_STREAM_ENDED, usize::from(failed), 0) };
    }

    unsafe fn tick_connect_spinner(hwnd: HWND) {
        if let Some(state) = state(hwnd) {
            if let Ok(mut guard) = state.0.lock() {
                if !guard.connected || !guard.connecting {
                    KillTimer(hwnd, TIMER_CONNECT_SPINNER);
                    return;
                }
                let frame = SPINNER_FRAMES[guard.spinner_index % SPINNER_FRAMES.len()];
                guard.spinner_index = guard.spinner_index.wrapping_add(1);
                set_window_text(guard.status, &format!("LIVE connecting {frame}"));
            }
        }
    }

    unsafe fn stream_ended(hwnd: HWND, failed: bool) {
        if let Some(state) = state(hwnd) {
            if let Ok(mut guard) = state.0.lock() {
                if !guard.connected {
                    return;
                }
                guard.connected = false;
                guard.connecting = false;
                guard.stop_tx = None;
                KillTimer(hwnd, TIMER_CONNECT_SPINNER);
                KillTimer(hwnd, TIMER_LOG_DRAIN);
                PENDING_LOG_POSTED.store(false, Ordering::Release);
                set_window_text(guard.connect, "Connect");
                if failed {
                    set_window_text(guard.status, "stream ended");
                } else {
                    set_window_text(guard.status, "disconnected");
                }
            }
        }
    }

    unsafe fn clear_history(hwnd: HWND) {
        if let Some(state) = state(hwnd) {
            if let Ok(mut guard) = state.0.lock() {
                guard.events.clear();
                guard.visible_indices.clear();
                reset_listbox_rows(guard.logs, 0);
                set_window_text(guard.status, "history cleared");
            }
        }
    }

    unsafe fn open_log_file(hwnd: HWND) {
        if let Some(path) = choose_open_path(hwnd) {
            match crate::input::load_events_from_input(&path) {
                Ok(events) => {
                    if let Some(state) = state(hwnd) {
                        if let Ok(mut guard) = state.0.lock() {
                            guard.events = events;
                            guard.visible_indices.clear();
                            set_window_text(guard.status, "file loaded");
                        }
                    }
                    rebuild_log_list(hwnd);
                }
                Err(e) => message_box(hwnd, "Open failed", &e),
            }
        }
    }

    unsafe fn save_visible_logs(hwnd: HWND) {
        let Some(path) = choose_save_path(hwnd) else { return; };
        let Some(state_handle) = state(hwnd) else { return; };
        let payload = if let Ok(guard) = state_handle.0.lock() {
            let mut output = String::new();
            for index in &guard.visible_indices {
                if let Some(event) = guard.events.get(*index) {
                    match serde_json::to_string(event) {
                        Ok(line) => {
                            output.push_str(&line);
                            output.push('\n');
                        }
                        Err(e) => {
                            message_box(hwnd, "Save failed", &format!("serialize log event failed: {e}"));
                            return;
                        }
                    }
                }
            }
            output
        } else {
            return;
        };

        match fs::write(&path, payload) {
            Ok(()) => {
                if let Some(state_handle) = state(hwnd) {
                    if let Ok(guard) = state_handle.0.lock() {
                        set_window_text(guard.status, "visible logs saved");
                    }
                }
            }
            Err(e) => message_box(hwnd, "Save failed", &format!("write failed: {e}")),
        }
    }

    unsafe fn show_about(hwnd: HWND) {
        message_box(
            hwnd,
            "About North Star Log Reader",
            "Take Some - North Star\n\nNorth Star LIVE Log Reader\nFirst-party Logger Plugin diagnostics tool.",
        );
    }

    unsafe fn choose_open_path(hwnd: HWND) -> Option<String> {
        choose_file_path(
            hwnd,
            "Open log file",
            "Log files (*.jsonl;*.ulog.jsonl)\0*.jsonl;*.ulog.jsonl\0All files (*.*)\0*.*\0\0",
            "jsonl",
            false,
        )
    }

    unsafe fn choose_save_path(hwnd: HWND) -> Option<String> {
        choose_file_path(
            hwnd,
            "Save visible logs",
            "JSONL logs (*.jsonl)\0*.jsonl\0All files (*.*)\0*.*\0\0",
            "jsonl",
            true,
        )
    }

    unsafe fn choose_file_path(hwnd: HWND, title: &str, filter: &str, default_ext: &str, save: bool) -> Option<String> {
        let mut file = vec![0u16; 4096];
        let title = wstr(title);
        let filter = wstr(filter);
        let default_ext = wstr(default_ext);
        let mut ofn = OPENFILENAMEW::default();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = file.as_mut_ptr();
        ofn.nMaxFile = file.len() as u32;
        ofn.lpstrTitle = title.as_ptr();
        ofn.lpstrDefExt = default_ext.as_ptr();
        ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_PATHMUSTEXIST;
        if save {
            ofn.Flags |= OFN_OVERWRITEPROMPT;
        } else {
            ofn.Flags |= OFN_FILEMUSTEXIST;
        }

        let ok = if save { GetSaveFileNameW(&mut ofn) } else { GetOpenFileNameW(&mut ofn) };
        if ok == 0 {
            return None;
        }
        let len = file.iter().position(|c| *c == 0).unwrap_or(file.len());
        Some(String::from_utf16_lossy(&file[..len]))
    }

    unsafe fn message_box(hwnd: HWND, title: &str, message: &str) {
        let title = wstr(title);
        let message = wstr(message);
        MessageBoxW(hwnd, message.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION);
    }

    fn pending_log_events() -> &'static Mutex<Vec<NormalizedLogEvent>> {
        PENDING_LOG_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
    }

    fn queue_log_event(hwnd: HWND, event: NormalizedLogEvent) {
        if let Ok(mut queue) = pending_log_events().lock() {
            queue.push(event);
            if queue.len() > MAX_PENDING_LOG_EVENTS {
                let overflow = queue.len() - MAX_PENDING_LOG_EVENTS;
                queue.drain(0..overflow);
            }
        }
        schedule_log_batch(hwnd);
    }

    fn schedule_log_batch(hwnd: HWND) {
        if !PENDING_LOG_POSTED.swap(true, Ordering::AcqRel) {
            // SAFETY: posts one wake-up; continued draining is timer-paced to keep input/drag responsive.
            unsafe { PostMessageW(hwnd, WM_LOG_EVENT, 0, 0) };
        }
    }

    unsafe fn drain_pending_log_events(hwnd: HWND) {
        let events = if let Ok(mut queue) = pending_log_events().lock() {
            let take = queue.len().min(MAX_UI_BATCH_EVENTS);
            queue.drain(..take).collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        if !events.is_empty() {
            append_event_batch(hwnd, events);
        }

        let has_more = pending_log_events()
            .lock()
            .map(|queue| !queue.is_empty())
            .unwrap_or(false);

        if has_more {
            // Do not immediately PostMessage again: that starves normal window input and causes "Not responding".
            SetTimer(hwnd, TIMER_LOG_DRAIN, LOG_DRAIN_INTERVAL_MS, None);
            PENDING_LOG_POSTED.store(true, Ordering::Release);
        } else {
            KillTimer(hwnd, TIMER_LOG_DRAIN);
            PENDING_LOG_POSTED.store(false, Ordering::Release);
        }
    }

    unsafe fn append_event_batch(hwnd: HWND, events: Vec<NormalizedLogEvent>) {
        if events.is_empty() {
            return;
        }
        let Some(state) = state(hwnd) else { return; };

        let (level_hwnd, search_hwnd, logs_hwnd, status_hwnd) = if let Ok(guard) = state.0.lock() {
            (guard.level, guard.search, guard.logs, guard.status)
        } else {
            return;
        };
        let min_level = selected_level(level_hwnd);
        let query = get_window_text(search_hwnd).to_lowercase();

        let mut list_reset = false;
        let mut rows_to_add = 0usize;
        let status_text = if let Ok(mut guard) = state.0.lock() {
            if guard.connecting {
                guard.connecting = false;
                KillTimer(hwnd, TIMER_CONNECT_SPINNER);
            }

            let start_index = guard.events.len();
            guard.events.extend(events);

            if guard.events.len() > MAX_RETAINED_LOG_EVENTS {
                let overflow = guard.events.len() - MAX_RETAINED_LOG_EVENTS;
                guard.events.drain(0..overflow);
                guard.visible_indices = compute_visible_indices(&guard.events, &min_level, &query);
                list_reset = true;
                rows_to_add = guard.visible_indices.len();
            } else {
                for index in start_index..guard.events.len() {
                    if event_matches(&guard.events[index], &min_level, &query) {
                        guard.visible_indices.push(index);
                        rows_to_add += 1;
                    }
                }
            }

            status_line_text(&guard)
        } else {
            return;
        };

        if list_reset {
            reset_listbox_rows(logs_hwnd, rows_to_add);
        } else {
            add_listbox_rows(logs_hwnd, rows_to_add);
        }
        set_window_text(status_hwnd, &status_text);
    }

    unsafe fn add_listbox_rows(logs_hwnd: HWND, rows: usize) {
        if rows == 0 {
            return;
        }

        let was_at_bottom = listbox_is_at_bottom(logs_hwnd);
        let text = wstr("");

        SendMessageW(logs_hwnd, WM_SETREDRAW_LOCAL, 0, 0);
        for _ in 0..rows {
            SendMessageW(logs_hwnd, LB_ADDSTRING, 0, text.as_ptr() as LPARAM);
        }
        if was_at_bottom {
            scroll_listbox_to_bottom(logs_hwnd);
        }
        SendMessageW(logs_hwnd, WM_SETREDRAW_LOCAL, 1, 0);
        repaint_without_erase(logs_hwnd);
    }

    unsafe fn reset_listbox_rows(logs_hwnd: HWND, rows: usize) {
        SendMessageW(logs_hwnd, WM_SETREDRAW_LOCAL, 0, 0);
        SendMessageW(logs_hwnd, LB_RESETCONTENT, 0, 0);
        let text = wstr("");
        for _ in 0..rows {
            SendMessageW(logs_hwnd, LB_ADDSTRING, 0, text.as_ptr() as LPARAM);
        }
        scroll_listbox_to_bottom(logs_hwnd);
        SendMessageW(logs_hwnd, WM_SETREDRAW_LOCAL, 1, 0);
        repaint_without_erase(logs_hwnd);
    }

    unsafe fn listbox_is_at_bottom(logs_hwnd: HWND) -> bool {
        let count = SendMessageW(logs_hwnd, LB_GETCOUNT, 0, 0) as i32;
        if count <= 0 {
            return true;
        }
        let top = SendMessageW(logs_hwnd, LB_GETTOPINDEX, 0, 0) as i32;
        let mut rect = std::mem::zeroed::<RECT>();
        if GetClientRect(logs_hwnd, &mut rect) == 0 {
            return true;
        }
        let visible_rows = ((rect.bottom - rect.top).max(1) / 24).max(1);
        top + visible_rows >= count - 1
    }

    unsafe fn scroll_listbox_to_bottom(logs_hwnd: HWND) {
        let count = SendMessageW(logs_hwnd, LB_GETCOUNT, 0, 0) as i32;
        if count <= 0 {
            return;
        }
        let mut rect = std::mem::zeroed::<RECT>();
        let visible_rows = if GetClientRect(logs_hwnd, &mut rect) != 0 {
            ((rect.bottom - rect.top).max(1) / 24).max(1)
        } else {
            1
        };
        let top = (count - visible_rows).max(0) as WPARAM;
        if SendMessageW(logs_hwnd, LB_GETTOPINDEX, 0, 0) != top as isize {
            SendMessageW(logs_hwnd, LB_SETTOPINDEX, top, 0);
        }
    }

    unsafe fn repaint_without_erase(hwnd: HWND) {
        RedrawWindow(
            hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_NOERASE | RDW_NOCHILDREN,
        );
    }

    unsafe fn rebuild_log_list(hwnd: HWND) {
        let Some(state) = state(hwnd) else { return; };
        let (level_hwnd, search_hwnd, logs_hwnd, status_hwnd) = if let Ok(guard) = state.0.lock() {
            (guard.level, guard.search, guard.logs, guard.status)
        } else {
            return;
        };
        let min_level = selected_level(level_hwnd);
        let query = get_window_text(search_hwnd).to_lowercase();

        let (rows, status_text) = if let Ok(mut guard) = state.0.lock() {
            guard.visible_indices = compute_visible_indices(&guard.events, &min_level, &query);
            (guard.visible_indices.len(), status_line_text(&guard))
        } else {
            return;
        };

        reset_listbox_rows(logs_hwnd, rows);
        set_window_text(status_hwnd, &status_text);
    }

    fn compute_visible_indices(events: &[NormalizedLogEvent], min_level: &str, query: &str) -> Vec<usize> {
        events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| event_matches(event, min_level, query).then_some(index))
            .collect()
    }

    fn status_line_text(guard: &AppState) -> String {
        let mode = if guard.connected {
            if guard.connecting { "LIVE connecting" } else { "LIVE connected" }
        } else {
            "disconnected"
        };
        format!("{mode} · {}/{} visible", guard.visible_indices.len(), guard.events.len())
    }

    fn event_matches(event: &NormalizedLogEvent, min_level: &str, query: &str) -> bool {
        if level_rank(&event.level) < level_rank(min_level) {
            return false;
        }
        if query.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {} {}",
            event.timestamp, event.level, event.target, event.event_id, event.message
        )
        .to_lowercase();
        haystack.contains(query)
    }

    unsafe fn draw_log_item(hwnd: HWND, draw: *const DRAWITEMSTRUCT) -> LRESULT {
        if draw.is_null() {
            return 0;
        }
        let draw = &*draw;
        if draw.CtlID != ID_LOGS as u32 || draw.itemID == u32::MAX {
            return 0;
        }

        let Some(state) = state(hwnd) else { return 1; };
        let event = if let Ok(guard) = state.0.lock() {
            guard
                .visible_indices
                .get(draw.itemID as usize)
                .and_then(|index| guard.events.get(*index))
                .cloned()
        } else {
            None
        };
        let Some(event) = event else { return 1; };

        let selected = (draw.itemState & ODS_SELECTED_LOCAL) != 0;
        let bg = if selected { rgb(39, 68, 103) } else { level_background(&event.level) };
        let brush = CreateSolidBrush(bg);
        FillRect(draw.hDC, &draw.rcItem, brush);
        DeleteObject(brush as _);
        SetBkMode(draw.hDC, TRANSPARENT as i32);

        let mut rect = draw.rcItem;
        rect.left += 8;
        rect.right -= 8;
        rect.top += 2;
        rect.bottom -= 2;

        let width = rect.right - rect.left;
        let time_w = width.min(220);
        let level_w = 62;
        let target_w = (width - time_w - level_w - 24).max(160).min(360);

        let mut time_rect = rect;
        time_rect.right = time_rect.left + time_w;
        draw_text(draw.hDC, &mut time_rect, &event.timestamp, rgb(146, 166, 192));

        let mut level_rect = rect;
        level_rect.left = time_rect.right + 8;
        level_rect.right = level_rect.left + level_w;
        draw_text(draw.hDC, &mut level_rect, &event.level, level_color(&event.level));

        let mut target_rect = rect;
        target_rect.left = level_rect.right + 8;
        target_rect.right = (target_rect.left + target_w).min(rect.right);
        draw_text(draw.hDC, &mut target_rect, &event.target, rgb(186, 208, 235));

        let mut msg_rect = rect;
        msg_rect.left = target_rect.right + 8;
        if msg_rect.left < msg_rect.right {
            draw_text(draw.hDC, &mut msg_rect, &event.message.replace('\n', " "), rgb(220, 231, 245));
        }
        1
    }

    unsafe fn draw_text(hdc: HDC, rect: &mut RECT, text: &str, color: u32) {
        SetTextColor(hdc, color);
        let text = wstr(text);
        DrawTextW(
            hdc,
            text.as_ptr(),
            -1,
            rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
    }

    fn level_color(level: &str) -> u32 {
        match level.to_ascii_uppercase().as_str() {
            "ERROR" => rgb(255, 107, 107),
            "WARN" => rgb(255, 209, 102),
            "INFO" => rgb(142, 202, 230),
            "DEBUG" => rgb(167, 201, 87),
            "TRACE" => rgb(157, 141, 241),
            _ => rgb(216, 226, 240),
        }
    }

    fn level_background(level: &str) -> u32 {
        match level.to_ascii_uppercase().as_str() {
            "ERROR" => rgb(42, 17, 24),
            "WARN" => rgb(38, 31, 13),
            "INFO" => rgb(10, 18, 29),
            "DEBUG" => rgb(14, 28, 18),
            "TRACE" => rgb(22, 18, 36),
            _ => rgb(10, 16, 25),
        }
    }

    const fn rgb(r: u8, g: u8, b: u8) -> u32 {
        (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
    }

    unsafe fn selected_level(level_hwnd: HWND) -> String {
        match SendMessageW(level_hwnd, CB_GETCURSEL, 0, 0) as i32 {
            0 => "TRACE".to_owned(),
            1 => "DEBUG".to_owned(),
            2 => "INFO".to_owned(),
            3 => "WARN".to_owned(),
            4 => "ERROR".to_owned(),
            _ => "INFO".to_owned(),
        }
    }

    fn level_rank(level: &str) -> u8 {
        match level {
            "ERROR" | "Error" | "error" => 5,
            "WARN" | "Warn" | "warn" => 4,
            "INFO" | "Info" | "info" => 3,
            "DEBUG" | "Debug" | "debug" => 2,
            "TRACE" | "Trace" | "trace" => 1,
            _ => 3,
        }
    }

    unsafe fn child(hwnd: HWND, class: &str, text: &str, style: u32, id: i32, hinstance: HINSTANCE) -> HWND {
        let class = wstr(class);
        let text = wstr(text);
        CreateWindowExW(
            0,
            class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            10,
            10,
            hwnd,
            id as isize as _,
            hinstance,
            null(),
        )
    }

    unsafe fn state(hwnd: HWND) -> Option<AppStateHandle> {
        raw_state_ptr(hwnd).map(|ptr| (*ptr).clone())
    }

    unsafe fn raw_state_ptr(hwnd: HWND) -> Option<*mut AppStateHandle> {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppStateHandle;
        if ptr.is_null() { None } else { Some(ptr) }
    }

    unsafe fn set_window_text(hwnd: HWND, text: &str) {
        let text = wstr(text);
        SetWindowTextW(hwnd, text.as_ptr());
    }

    unsafe fn get_window_text(hwnd: HWND) -> String {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let read = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..read as usize])
    }

    fn wstr(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
pub fn run(initial_url: &str) -> Result<(), String> {
    win32_app::run(initial_url)
}

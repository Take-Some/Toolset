#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("northstar-nepak-gui is currently implemented for Windows native GUI hosts only.");
}

#[cfg(windows)]
mod win_gui {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};
    use std::sync::{Mutex, OnceLock};

    use northstar_nepak_manager::nepak::{self, EntryKind, PackageEntry, ParsedPackage};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
        OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    };
    use windows_sys::Win32::UI::Controls::{
        HTREEITEM, NMHDR, NMTREEVIEWW, TVIF_PARAM, TVIF_TEXT, TVINSERTSTRUCTW, TVITEMW,
        TVM_DELETEITEM, TVM_INSERTITEMW, TVN_SELCHANGEDW, TVS_HASBUTTONS, TVS_HASLINES,
        TVS_LINESATROOT, TVS_SHOWSELALWAYS, TVI_LAST, TVI_ROOT, WC_TREEVIEWW,
    };
    use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const IDC_OPEN: i32 = 1001;
    const IDC_VERIFY: i32 = 1002;
    const IDC_EXTRACT: i32 = 1003;
    const IDC_EXTRACT_ALL: i32 = 1004;
    const IDC_PROVIDER: i32 = 1005;
    const IDC_TREE: i32 = 2001;
    const IDC_DETAILS: i32 = 2002;
    const IDC_PREVIEW: i32 = 2003;
    const IDC_STATUS: i32 = 2004;

    #[derive(Default)]
    struct AppState {
        hwnd: HWND,
        tree: HWND,
        details: HWND,
        preview: HWND,
        status: HWND,
        file_path: Option<PathBuf>,
        bytes: Vec<u8>,
        parsed: Option<ParsedPackage>,
        selected_entry: Option<usize>,
    }

    unsafe impl Send for AppState {}
    static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();

    pub fn run() {
        unsafe {
            let instance = GetModuleHandleW(null());
            let class_name = wide("NorthStarNepakGuiWindow");
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: instance,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: 6 as _,
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                wide("North Star NEPAK GUI").as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1280,
                780,
                null_mut(),
                null_mut(),
                instance,
                null_mut(),
            );
            if hwnd.is_null() {
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_CREATE => { create_controls(hwnd); 0 }
            WM_SIZE => { resize_controls(hwnd); 0 }
            WM_DROPFILES => { handle_drop(wparam as HDROP); 0 }
            WM_NOTIFY => { handle_notify(lparam); 0 }
            WM_COMMAND => {
                match loword(wparam as usize) as i32 {
                    IDC_OPEN => open_package(hwnd),
                    IDC_VERIFY => verify_package(),
                    IDC_EXTRACT => extract_selected(hwnd),
                    IDC_EXTRACT_ALL => extract_all(),
                    IDC_PROVIDER => open_selected_in_provider(),
                    _ => {}
                }
                0
            }
            WM_DESTROY => { PostQuitMessage(0); 0 }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn create_controls(hwnd: HWND) {
        let font = GetStockObject(DEFAULT_GUI_FONT as i32);
        let btn_style = WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON as u32;
        let open = child(hwnd, "BUTTON", "Open .nepak", btn_style, IDC_OPEN);
        let verify = child(hwnd, "BUTTON", "Verify", btn_style, IDC_VERIFY);
        let extract = child(hwnd, "BUTTON", "Extract selected", btn_style, IDC_EXTRACT);
        let extract_all = child(hwnd, "BUTTON", "Extract all", btn_style, IDC_EXTRACT_ALL);
        let provider = child(hwnd, "BUTTON", "Open in provider", btn_style, IDC_PROVIDER);
        let tree = child_pcw(hwnd, WC_TREEVIEWW, "", WS_CHILD | WS_VISIBLE | WS_BORDER | WS_VSCROLL | WS_HSCROLL | TVS_HASLINES | TVS_HASBUTTONS | TVS_LINESATROOT | TVS_SHOWSELALWAYS, IDC_TREE);
        let details = child(hwnd, "EDIT", "Open or drag-drop a .nepak package.", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_READONLY as u32 | WS_VSCROLL, IDC_DETAILS);
        let preview = child(hwnd, "EDIT", "Hex/ASCII payload preview will appear here.", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_READONLY as u32 | WS_VSCROLL | WS_HSCROLL, IDC_PREVIEW);
        let status = child(hwnd, "STATIC", "Ready. Drag-drop .nepak files is enabled.", WS_CHILD | WS_VISIBLE, IDC_STATUS);
        for control in [open, verify, extract, extract_all, provider, tree, details, preview, status] {
            SendMessageW(control, WM_SETFONT, font as WPARAM, 1);
        }
        DragAcceptFiles(hwnd, 1);
        let _ = STATE.set(Mutex::new(AppState { hwnd, tree, details, preview, status, ..Default::default() }));
        resize_controls(hwnd);
    }

    unsafe fn resize_controls(hwnd: HWND) {
        let mut rc = RECT::default();
        GetClientRect(hwnd, &mut rc);
        let w = rc.right - rc.left;
        let h = rc.bottom - rc.top;
        let margin = 10;
        let button_h = 30;
        let status_h = 24;
        let tree_w = (w * 38 / 100).max(360);
        let right_x = margin + tree_w + 10;
        let right_w = w - right_x - margin;
        let body_y = margin + button_h + 10;
        let body_h = h - body_y - status_h - 16;
        let details_h = (body_h * 48 / 100).max(160);
        if let Some(m) = STATE.get() {
            if let Ok(state) = m.lock() {
                MoveWindow(GetDlgItem(hwnd, IDC_OPEN), margin, margin, 130, button_h, 1);
                MoveWindow(GetDlgItem(hwnd, IDC_VERIFY), margin + 140, margin, 90, button_h, 1);
                MoveWindow(GetDlgItem(hwnd, IDC_EXTRACT), margin + 240, margin, 145, button_h, 1);
                MoveWindow(GetDlgItem(hwnd, IDC_EXTRACT_ALL), margin + 395, margin, 110, button_h, 1);
                MoveWindow(GetDlgItem(hwnd, IDC_PROVIDER), margin + 515, margin, 145, button_h, 1);
                MoveWindow(state.tree, margin, body_y, tree_w, body_h, 1);
                MoveWindow(state.details, right_x, body_y, right_w, details_h, 1);
                MoveWindow(state.preview, right_x, body_y + details_h + 10, right_w, body_h - details_h - 10, 1);
                MoveWindow(state.status, margin, h - status_h - 6, w - 20, status_h, 1);
            }
        }
    }

    unsafe fn open_package(hwnd: HWND) {
        if let Some(path) = file_dialog(hwnd, false, "NEPAK packages\0*.nepak\0All files\0*.*\0", "nepak", None) {
            load_package(&path, hwnd);
        }
    }

    unsafe fn load_package(path: &Path, hwnd: HWND) {
        match fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| {
            let parsed = nepak::parse(&bytes)?;
            Ok((bytes, parsed))
        }) {
            Ok((bytes, parsed)) => {
                if let Some(m) = STATE.get() {
                    let mut state = m.lock().unwrap();
                    state.file_path = Some(path.to_path_buf());
                    state.bytes = bytes;
                    state.parsed = Some(parsed);
                    state.selected_entry = None;
                    rebuild_tree(&mut state);
                }
                set_status(&format!("Opened {}", path.display()));
            }
            Err(err) => {
                set_status(&format!("Open failed: {err}"));
                message(hwnd, "Open failed", &err);
            }
        }
    }

    unsafe fn rebuild_tree(state: &mut AppState) {
        SendMessageW(state.tree, TVM_DELETEITEM, 0, TVI_ROOT as LPARAM);
        let Some(parsed) = &state.parsed else { return; };
        let mut handles = vec![0isize; parsed.entries.len()];
        for (idx, entry) in parsed.entries.iter().enumerate() {
            let parent = if entry.parent_index == u32::MAX { TVI_ROOT } else { handles[entry.parent_index as usize] };
            let label = tree_label(entry);
            handles[idx] = insert_tree_item(state.tree, parent, &label, idx as isize);
        }
        let summary = package_summary(parsed, state.file_path.as_deref());
        SetWindowTextW(state.details, wide(&summary).as_ptr());
        SetWindowTextW(state.preview, wide("Select a file/resource entry to preview payload bytes.").as_ptr());
    }

    unsafe fn insert_tree_item(tree: HWND, parent: HTREEITEM, label: &str, param: isize) -> HTREEITEM {
        let mut text = wide(label);
        let item = TVITEMW {
            mask: TVIF_TEXT | TVIF_PARAM,
            pszText: text.as_mut_ptr(),
            cchTextMax: label.len() as i32,
            lParam: param as LPARAM,
            ..Default::default()
        };
        let mut ins = TVINSERTSTRUCTW { hParent: parent, hInsertAfter: TVI_LAST, Anonymous: Default::default() };
        ins.Anonymous.item = item;
        SendMessageW(tree, TVM_INSERTITEMW, 0, &ins as *const TVINSERTSTRUCTW as LPARAM) as HTREEITEM
    }

    fn tree_label(e: &PackageEntry) -> String {
        if e.entry_kind == EntryKind::Directory {
            format!("[DIR] {}", if e.path.is_empty() { "/" } else { &e.name })
        } else {
            format!("{}  [{} | {} -> {}]", e.name, e.storage_class.as_str_public(), e.stored_size, e.decoded_size)
        }
    }

    unsafe fn handle_notify(lparam: LPARAM) {
        if lparam == 0 { return; }
        let hdr = &*(lparam as *const NMHDR);
        if hdr.code == TVN_SELCHANGEDW {
            let tv = &*(lparam as *const NMTREEVIEWW);
            let idx = tv.itemNew.lParam as usize;
            select_entry(idx);
        }
    }

    unsafe fn select_entry(idx: usize) {
        let Some(m) = STATE.get() else { return; };
        let mut state = m.lock().unwrap();
        state.selected_entry = Some(idx);
        let Some(parsed) = &state.parsed else { return; };
        let Some(e) = parsed.entries.get(idx) else { return; };
        let text = entry_details(e);
        SetWindowTextW(state.details, wide(&text).as_ptr());
        let preview = if e.entry_kind == EntryKind::Directory {
            String::from("Directory entry: no payload bytes.")
        } else {
            match nepak::read_entry_bytes(&state.bytes, &e.path) {
                Ok(raw) => hex_preview(&raw, 1024),
                Err(err) => format!("Preview failed: {err}"),
            }
        };
        SetWindowTextW(state.preview, wide(&preview).as_ptr());
    }

    fn package_summary(parsed: &ParsedPackage, path: Option<&Path>) -> String {
        let files = parsed.entries.iter().filter(|e| e.entry_kind != EntryKind::Directory).count();
        let dirs = parsed.entries.iter().filter(|e| e.entry_kind == EntryKind::Directory).count();
        let stored: u64 = parsed.entries.iter().map(|e| e.stored_size).sum();
        let decoded: u64 = parsed.entries.iter().map(|e| e.decoded_size).sum();
        format!(
            "NEPAK binary package\r\n\r\nFile: {}\r\nLayout: header -> entry table -> name table -> sector-aligned data\r\nEntries: {}\r\nDirectories: {}\r\nFiles/resources: {}\r\nData offset: {}\r\nData size: {}\r\nTotal stored payload: {}\r\nTotal decoded payload: {}\r\n\r\nTreeView is built from the binary central directory. JSON is allowed as payload bytes, not as internal container metadata.",
            path.map(|p| p.display().to_string()).unwrap_or_else(|| "<none>".to_owned()),
            parsed.entries.len(), dirs, files, parsed.data_offset, parsed.data_size, stored, decoded
        )
    }

    fn entry_details(e: &PackageEntry) -> String {
        format!(
            "Entry metadata\r\n\r\nIndex: {}\r\nName: {}\r\nPath: {}\r\nKind: {}\r\nContent kind: {}\r\nStorage class: {}\r\nCompression: {}\r\nData sector: {}\r\nByte offset: {}\r\nStored size: {}\r\nDecoded size: {}\r\nVirtual size: {}\r\nPhysical size: {}\r\nVirtual chunks: {}\r\nPhysical chunks: {}\r\nHash: {}\r\n\r\nPayload bytes are opaque. Domain formats such as .ytd/.ydd/.ytyp/.nemat are routed to their own providers.",
            e.index, e.name, if e.path.is_empty() { "/" } else { &e.path }, e.entry_kind.as_str_public(), e.content_kind.as_str_public(), e.storage_class.as_str_public(), e.compression_label(), e.data_sector, e.byte_offset, e.stored_size, e.decoded_size, e.resource_layout.virtual_size, e.resource_layout.physical_size, e.resource_layout.virtual_chunk_count, e.resource_layout.physical_chunk_count, hex32(&e.hash)
        )
    }

    fn hex_preview(raw: &[u8], max: usize) -> String {
        let shown = raw.len().min(max);
        let mut out = String::new();
        out.push_str(&format!("Payload preview: showing {} of {} decoded bytes\r\n\r\n", shown, raw.len()));
        for (row, chunk) in raw[..shown].chunks(16).enumerate() {
            out.push_str(&format!("{:08x}  ", row * 16));
            for i in 0..16 {
                if let Some(b) = chunk.get(i) { out.push_str(&format!("{:02x} ", b)); } else { out.push_str("   "); }
                if i == 7 { out.push(' '); }
            }
            out.push_str(" | ");
            for b in chunk {
                let c = if (0x20..=0x7e).contains(b) { *b as char } else { '.' };
                out.push(c);
            }
            out.push_str("\r\n");
        }
        out
    }

    unsafe fn verify_package() {
        let Some(m) = STATE.get() else { return; };
        let state = m.lock().unwrap();
        if state.bytes.is_empty() { set_status("No package loaded"); return; }
        match nepak::validate_bytes(&state.bytes) {
            Ok(count) => set_status(&format!("Verify OK: {count} file/resource entries")),
            Err(err) => set_status(&format!("Verify failed: {err}")),
        }
    }

    unsafe fn extract_selected(hwnd: HWND) {
        let Some(m) = STATE.get() else { return; };
        let (bytes, path, default_name) = {
            let state = m.lock().unwrap();
            let Some(idx) = state.selected_entry else { set_status("Select an entry first"); return; };
            let Some(parsed) = &state.parsed else { return; };
            let e = &parsed.entries[idx];
            if e.entry_kind == EntryKind::Directory { set_status("Cannot extract a directory as selected file"); return; }
            (state.bytes.clone(), e.path.clone(), e.name.clone())
        };
        if let Some(out) = file_dialog(hwnd, true, "All files\0*.*\0", "", Some(&default_name)) {
            match nepak::read_entry_bytes(&bytes, &path).and_then(|raw| fs::write(&out, raw).map_err(|e| e.to_string())) {
                Ok(()) => set_status(&format!("Extracted {} -> {}", path, out.display())),
                Err(err) => { set_status(&format!("Extract failed: {err}")); message(hwnd, "Extract failed", &err); }
            }
        }
    }

    unsafe fn extract_all() {
        let Some(m) = STATE.get() else { return; };
        let (bytes, root) = {
            let state = m.lock().unwrap();
            if state.bytes.is_empty() { set_status("No package loaded"); return; }
            let root = state.file_path.as_ref().map(|p| p.with_extension("nepak.extract")).unwrap_or_else(|| PathBuf::from("nepak.extract"));
            (state.bytes.clone(), root)
        };
        match nepak::extract_to(&bytes, &root, None, true) {
            Ok(count) => set_status(&format!("Extracted {count} entries -> {}", root.display())),
            Err(err) => set_status(&format!("Extract all failed: {err}")),
        }
    }

    unsafe fn open_selected_in_provider() {
        let Some(m) = STATE.get() else { return; };
        let state = m.lock().unwrap();
        let Some(idx) = state.selected_entry else { set_status("Select an entry first"); return; };
        let Some(parsed) = &state.parsed else { return; };
        let e = &parsed.entries[idx];
        let ext = Path::new(&e.path).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        let provider = match ext.as_str() {
            "ytd" => "northstar.ytd_packer / texture dictionary provider",
            "ydd" => "northstar.ydd_packer / drawable dictionary provider",
            "ytyp" => "northstar.ytyp_packer / archetype metadata provider",
            "nemat" => "northstar.nemat_packer / material library provider",
            _ => "no domain provider registered for this extension",
        };
        set_status(&format!("Provider route for {}: {}", e.path, provider));
    }

    unsafe fn handle_drop(drop: HDROP) {
        let mut buf = vec![0u16; 4096];
        let copied = DragQueryFileW(drop, 0, buf.as_mut_ptr(), buf.len() as u32);
        DragFinish(drop);
        if copied == 0 { return; }
        let path = PathBuf::from(String::from_utf16_lossy(&buf[..copied as usize]));
        if path.extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("nepak")).unwrap_or(false) {
            let hwnd = STATE.get().and_then(|m| m.lock().ok().map(|s| s.hwnd)).unwrap_or(null_mut());
            load_package(&path, hwnd);
        } else {
            set_status("Dropped file is not a .nepak package");
        }
    }

    unsafe fn set_status(text: &str) {
        if let Some(m) = STATE.get() {
            if let Ok(state) = m.lock() { SetWindowTextW(state.status, wide(text).as_ptr()); }
        }
    }

    unsafe fn child(hwnd: HWND, class: &str, text: &str, style: u32, id: i32) -> HWND {
        CreateWindowExW(0, wide(class).as_ptr(), wide(text).as_ptr(), style, 0, 0, 0, 0, hwnd, id as usize as _, GetModuleHandleW(null()), null_mut())
    }
    unsafe fn child_pcw(hwnd: HWND, class: windows_sys::core::PCWSTR, text: &str, style: u32, id: i32) -> HWND {
        CreateWindowExW(0, class, wide(text).as_ptr(), style, 0, 0, 0, 0, hwnd, id as usize as _, GetModuleHandleW(null()), null_mut())
    }

    unsafe fn file_dialog(hwnd: HWND, save: bool, filter: &str, default_ext: &str, default_name: Option<&str>) -> Option<PathBuf> {
        let mut file = vec![0u16; 4096];
        if let Some(name) = default_name {
            let w = wide(name);
            let n = w.len().saturating_sub(1).min(file.len().saturating_sub(1));
            file[..n].copy_from_slice(&w[..n]);
        }
        let filter = wide(filter);
        let default_ext = wide(default_ext);
        let mut ofn = OPENFILENAMEW::default();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = file.as_mut_ptr();
        ofn.nMaxFile = file.len() as u32;
        ofn.lpstrDefExt = default_ext.as_ptr();
        ofn.Flags = OFN_NOCHANGEDIR | OFN_HIDEREADONLY | if save { OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST } else { OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST };
        let ok = if save { GetSaveFileNameW(&mut ofn) } else { GetOpenFileNameW(&mut ofn) };
        if ok == 0 { return None; }
        let len = file.iter().position(|c| *c == 0).unwrap_or(file.len());
        Some(PathBuf::from(String::from_utf16_lossy(&file[..len])))
    }

    unsafe fn message(hwnd: HWND, title: &str, text: &str) { MessageBoxW(hwnd, wide(text).as_ptr(), wide(title).as_ptr(), MB_OK | MB_ICONERROR); }
    fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
    fn loword(v: usize) -> u16 { (v & 0xffff) as u16 }
    fn hex32(v: &[u8; 32]) -> String { v.iter().map(|b| format!("{b:02x}")).collect() }

    trait GuiLabels { fn compression_label(&self) -> &'static str; }
    impl GuiLabels for PackageEntry { fn compression_label(&self) -> &'static str { match self.compression { nepak::CompressionKind::None => "none", nepak::CompressionKind::Deflate => "deflate" } } }
    trait EntryKindLabel { fn as_str_public(self) -> &'static str; }
    impl EntryKindLabel for EntryKind { fn as_str_public(self) -> &'static str { match self { EntryKind::Directory => "directory", EntryKind::File => "file", EntryKind::Resource => "resource" } } }
    trait ContentKindLabel { fn as_str_public(self) -> &'static str; }
    impl ContentKindLabel for nepak::ContentKind {
        fn as_str_public(self) -> &'static str {
            match self {
                nepak::ContentKind::OpaqueFile => "opaque_file",
                nepak::ContentKind::VfsPackage => "vfs_package",
                nepak::ContentKind::TextureDictionary => "texture_dictionary",
                nepak::ContentKind::DrawableDictionary => "drawable_dictionary",
                nepak::ContentKind::ArchetypeDictionary => "archetype_dictionary",
                nepak::ContentKind::MaterialLibrary => "material_library",
                nepak::ContentKind::AiPatternDictionary => "ai_pattern_dictionary",
                nepak::ContentKind::UiDocument => "ui_document",
            }
        }
    }
    trait StorageLabel { fn as_str_public(self) -> &'static str; }
    impl StorageLabel for nepak::StorageClass { fn as_str_public(self) -> &'static str { match self { nepak::StorageClass::RawFile => "raw_file", nepak::StorageClass::Directory => "directory", nepak::StorageClass::ListFile => "listfile" } } }
}

#[cfg(windows)]
fn main() { win_gui::run(); }

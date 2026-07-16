#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-log-reader",
        original_filename: "northstar-log-reader.exe",
        file_description: "Ulogger",
        icon_path: Some("../icons/UniversalLog.ico"),
    });
}

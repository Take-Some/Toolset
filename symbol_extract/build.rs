#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-symbol-extract",
        original_filename: "northstar-symbol-extract.exe",
        file_description: "North Star Symbol Extract",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

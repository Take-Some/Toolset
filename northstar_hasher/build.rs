#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-hasher",
        original_filename: "northstar-hasher.exe",
        file_description: "North Star Hasher",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

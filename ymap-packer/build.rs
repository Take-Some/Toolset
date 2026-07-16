#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-ymap-packer",
        original_filename: "northstar-ymap-packer.exe",
        file_description: "North Star YMAP Packer",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

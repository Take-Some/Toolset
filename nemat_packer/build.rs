#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-nemat-packer",
        original_filename: "northstar-nemat-packer.exe",
        file_description: "North Star NEMAT material library tool",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

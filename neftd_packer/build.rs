#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-neftd-packer",
        original_filename: "northstar-neftd-packer.exe",
        file_description: "North Star NEFTD NEF8 font dictionary tool",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-yfd-packer",
        original_filename: "northstar-yfd-packer.exe",
        file_description: "North Star YFD NEF8 font dictionary tool",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-ytyp-packer",
        original_filename: "northstar-ytyp-packer.exe",
        file_description: "North Star YTYP Packer",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-ydd-packer",
        original_filename: "northstar-ydd-packer.exe",
        file_description: "North Star YDD NEF8 drawable dictionary importer",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

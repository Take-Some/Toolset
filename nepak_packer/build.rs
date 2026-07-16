#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-nepak-manager",
        original_filename: "northstar-nepak-manager.exe",
        file_description: "North Star clean NEPAK VFS package manager",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

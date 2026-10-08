#[path = "../../shared/northstar_build/windows_file_info.rs"]
mod windows_file_info;

fn main() {
    windows_file_info::compile(windows_file_info::ToolFileInfo {
        internal_name: "northstar-yscd-packer",
        original_filename: "northstar-yscd-packer.exe",
        file_description: "North Star Y Sound Cue Dictionary Packer",
        icon_path: Some("../icons/AppIcon.ico"),
    });
}

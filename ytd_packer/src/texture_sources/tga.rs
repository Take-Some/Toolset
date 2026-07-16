use newengine_texture_container::TextureEncodedBuildEntry;
use std::path::Path;

use super::common;

pub fn load(
    name: String,
    path: &Path,
    srgb: bool,
    no_mips: bool,
) -> Result<TextureEncodedBuildEntry, String> {
    common::load_raster_file("TGA", name, path, srgb, no_mips)
}

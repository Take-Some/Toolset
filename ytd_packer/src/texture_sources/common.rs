use image::GenericImageView;
use newengine_texture_container::{
    generate_rgba8_mips, TextureEncodedBuildEntry, TextureEncodedMipData, PIXEL_FORMAT_RGBA8_SRGB,
    PIXEL_FORMAT_RGBA8_UNORM,
};
use std::path::Path;

pub fn load_raster_file(
    label: &str,
    name: String,
    path: &Path,
    srgb: bool,
    no_mips: bool,
) -> Result<TextureEncodedBuildEntry, String> {
    let img = image::open(path)
        .map_err(|e| format!("{label} decode '{}' failed: {e}", path.display()))?;
    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8().into_raw();
    let entry = rgba8_entry(name, width, height, rgba, srgb, no_mips)?;
    northstar_cli::ansi::ok(format!(
        "source {label}: {} {}x{} format={} mips={}",
        path.display(),
        entry.width,
        entry.height,
        entry.format,
        entry.mips.len()
    ));
    Ok(entry)
}

pub fn rgba8_entry(
    name: String,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    srgb: bool,
    no_mips: bool,
) -> Result<TextureEncodedBuildEntry, String> {
    let mips = if no_mips {
        vec![TextureEncodedMipData {
            level: 0,
            width,
            height,
            bytes: rgba,
        }]
    } else {
        generate_rgba8_mips(width, height, rgba)
            .map_err(|e| format!("mip generation '{name}' failed: {e}"))?
            .into_iter()
            .map(|mip| TextureEncodedMipData {
                level: mip.level,
                width: mip.width,
                height: mip.height,
                bytes: mip.rgba,
            })
            .collect()
    };
    let format = if srgb {
        PIXEL_FORMAT_RGBA8_SRGB
    } else {
        PIXEL_FORMAT_RGBA8_UNORM
    }
    .to_owned();
    let color_space = if srgb { "srgb" } else { "linear" }.to_owned();
    Ok(TextureEncodedBuildEntry {
        name,
        width,
        height,
        format,
        color_space,
        mips,
    })
}

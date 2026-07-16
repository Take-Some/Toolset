use newengine_texture_container::{
    read_dds_runtime_texture, TextureEncodedBuildEntry, PIXEL_FORMAT_BC1_RGBA_SRGB,
    PIXEL_FORMAT_BC1_RGBA_UNORM, PIXEL_FORMAT_BC2_RGBA_SRGB, PIXEL_FORMAT_BC2_RGBA_UNORM,
    PIXEL_FORMAT_BC3_RGBA_SRGB, PIXEL_FORMAT_BC3_RGBA_UNORM, PIXEL_FORMAT_BC7_RGBA_SRGB,
    PIXEL_FORMAT_BC7_RGBA_UNORM, PIXEL_FORMAT_RGBA8_SRGB, PIXEL_FORMAT_RGBA8_UNORM,
};
use std::{fs, path::Path};

pub fn load(
    name: String,
    path: &Path,
    srgb: bool,
    no_mips: bool,
) -> Result<TextureEncodedBuildEntry, String> {
    let bytes = fs::read(path).map_err(|e| format!("read '{}' failed: {e}", path.display()))?;
    let mut dds = read_dds_runtime_texture(&bytes)
        .map_err(|e| format!("DDS import '{}' failed: {e}", path.display()))?;
    if no_mips && dds.mips.len() > 1 {
        dds.mips.truncate(1);
    }
    if srgb {
        dds.format = force_srgb_format(&dds.format);
        dds.color_space = "srgb".to_owned();
    } else {
        dds.format = force_linear_format(&dds.format);
        dds.color_space = "linear".to_owned();
    }
    northstar_cli::ansi::ok(format!(
        "source DDS: {} {}x{} format={} mips={}",
        path.display(),
        dds.width,
        dds.height,
        dds.format,
        dds.mips.len()
    ));
    Ok(TextureEncodedBuildEntry {
        name,
        width: dds.width,
        height: dds.height,
        format: dds.format,
        color_space: dds.color_space,
        mips: dds.mips,
    })
}

fn force_srgb_format(format: &str) -> String {
    match format {
        PIXEL_FORMAT_RGBA8_UNORM => PIXEL_FORMAT_RGBA8_SRGB.to_owned(),
        PIXEL_FORMAT_BC1_RGBA_UNORM => PIXEL_FORMAT_BC1_RGBA_SRGB.to_owned(),
        PIXEL_FORMAT_BC2_RGBA_UNORM => PIXEL_FORMAT_BC2_RGBA_SRGB.to_owned(),
        PIXEL_FORMAT_BC3_RGBA_UNORM => PIXEL_FORMAT_BC3_RGBA_SRGB.to_owned(),
        PIXEL_FORMAT_BC7_RGBA_UNORM => PIXEL_FORMAT_BC7_RGBA_SRGB.to_owned(),
        other => other.to_owned(),
    }
}

fn force_linear_format(format: &str) -> String {
    match format {
        PIXEL_FORMAT_RGBA8_SRGB => PIXEL_FORMAT_RGBA8_UNORM.to_owned(),
        PIXEL_FORMAT_BC1_RGBA_SRGB => PIXEL_FORMAT_BC1_RGBA_UNORM.to_owned(),
        PIXEL_FORMAT_BC2_RGBA_SRGB => PIXEL_FORMAT_BC2_RGBA_UNORM.to_owned(),
        PIXEL_FORMAT_BC3_RGBA_SRGB => PIXEL_FORMAT_BC3_RGBA_UNORM.to_owned(),
        PIXEL_FORMAT_BC7_RGBA_SRGB => PIXEL_FORMAT_BC7_RGBA_UNORM.to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_texture_container::PIXEL_FORMAT_BC5_RG_UNORM;

    #[test]
    fn color_space_overrides_preserve_linear_only_bc5() {
        assert_eq!(
            force_srgb_format(PIXEL_FORMAT_BC5_RG_UNORM),
            PIXEL_FORMAT_BC5_RG_UNORM
        );
        assert_eq!(
            force_linear_format(PIXEL_FORMAT_BC5_RG_UNORM),
            PIXEL_FORMAT_BC5_RG_UNORM
        );
    }

    #[test]
    fn color_space_overrides_convert_rgba8_and_bcn_variants() {
        assert_eq!(
            force_srgb_format(PIXEL_FORMAT_RGBA8_UNORM),
            PIXEL_FORMAT_RGBA8_SRGB
        );
        assert_eq!(
            force_linear_format(PIXEL_FORMAT_RGBA8_SRGB),
            PIXEL_FORMAT_RGBA8_UNORM
        );
        assert_eq!(
            force_srgb_format(PIXEL_FORMAT_BC3_RGBA_UNORM),
            PIXEL_FORMAT_BC3_RGBA_SRGB
        );
        assert_eq!(
            force_linear_format(PIXEL_FORMAT_BC3_RGBA_SRGB),
            PIXEL_FORMAT_BC3_RGBA_UNORM
        );
    }
}

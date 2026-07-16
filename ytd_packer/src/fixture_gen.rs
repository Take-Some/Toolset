use std::{fs, path::Path};

pub fn write_smoke_fixtures(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("create fixture dir '{}' failed: {e}", dir.display()))?;
    let rgba = sample_rgba();
    write_image_fixtures(dir, &rgba)?;
    let dds = rgba8_dds(&rgba);
    fs::write(dir.join("sample_dds.dds"), &dds).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_image_fixtures(dir: &Path, rgba: &[u8]) -> Result<(), String> {
    use image::{ColorType, ImageFormat};
    image::save_buffer_with_format(
        dir.join("sample_png.png"),
        rgba,
        4,
        4,
        ColorType::Rgba8,
        ImageFormat::Png,
    )
    .map_err(|e| format!("write png failed: {e}"))?;
    image::save_buffer_with_format(
        dir.join("sample_bmp.bmp"),
        rgba,
        4,
        4,
        ColorType::Rgba8,
        ImageFormat::Bmp,
    )
    .map_err(|e| format!("write bmp failed: {e}"))?;
    image::save_buffer_with_format(
        dir.join("sample_tga.tga"),
        rgba,
        4,
        4,
        ColorType::Rgba8,
        ImageFormat::Tga,
    )
    .map_err(|e| format!("write tga failed: {e}"))?;
    let rgb = rgba_to_rgb(rgba);
    image::save_buffer_with_format(
        dir.join("sample_jpg.jpg"),
        &rgb,
        4,
        4,
        ColorType::Rgb8,
        ImageFormat::Jpeg,
    )
    .map_err(|e| format!("write jpg failed: {e}"))?;
    image::save_buffer_with_format(
        dir.join("sample_jpeg.jpeg"),
        &rgb,
        4,
        4,
        ColorType::Rgb8,
        ImageFormat::Jpeg,
    )
    .map_err(|e| format!("write jpeg failed: {e}"))?;
    Ok(())
}

fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect()
}

fn sample_rgba() -> Vec<u8> {
    let mut rgba = Vec::with_capacity(4 * 4 * 4);
    for y in 0..4u8 {
        for x in 0..4u8 {
            rgba.extend_from_slice(&[x.saturating_mul(64), y.saturating_mul(64), 128, 255]);
        }
    }
    rgba
}

fn rgba8_dds(rgba: &[u8]) -> Vec<u8> {
    let mut out = b"DDS ".to_vec();
    push_u32(&mut out, 124);
    push_u32(&mut out, 0x0000_1007 | 0x0000_0008);
    push_u32(&mut out, 4);
    push_u32(&mut out, 4);
    push_u32(&mut out, 16);
    push_u32(&mut out, 0);
    push_u32(&mut out, 1);
    for _ in 0..11 {
        push_u32(&mut out, 0);
    }
    push_u32(&mut out, 32);
    push_u32(&mut out, 0x0000_0041);
    push_u32(&mut out, 0);
    push_u32(&mut out, 32);
    push_u32(&mut out, 0x0000_00ff);
    push_u32(&mut out, 0x0000_ff00);
    push_u32(&mut out, 0x00ff_0000);
    push_u32(&mut out, 0xff00_0000);
    push_u32(&mut out, 0x0000_1000);
    push_u32(&mut out, 0);
    push_u32(&mut out, 0);
    push_u32(&mut out, 0);
    push_u32(&mut out, 0);
    out.extend_from_slice(rgba);
    out
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

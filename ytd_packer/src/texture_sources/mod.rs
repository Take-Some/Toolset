pub mod bmp;
pub mod common;
pub mod dds;
pub mod jpeg;
pub mod jpg;
pub mod png;
pub mod tga;

use newengine_texture_container::TextureEncodedBuildEntry;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Png,
    Bmp,
    Jpg,
    Jpeg,
    Dds,
    Tga,
}

impl SourceKind {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        match path
            .extension()
            .and_then(|v| v.to_str())
            .map(|v| v.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") => Ok(Self::Png),
            Some("bmp") => Ok(Self::Bmp),
            Some("jpg") => Ok(Self::Jpg),
            Some("jpeg") => Ok(Self::Jpeg),
            Some("dds") => Ok(Self::Dds),
            Some("tga") => Ok(Self::Tga),
            Some(other) => Err(format!(
                "unsupported texture source extension '.{other}' for '{}'",
                path.display()
            )),
            None => Err(format!(
                "texture source '{}' has no extension",
                path.display()
            )),
        }
    }
}

pub fn load(
    kind: SourceKind,
    name: String,
    path: &Path,
    srgb: bool,
    no_mips: bool,
) -> Result<TextureEncodedBuildEntry, String> {
    match kind {
        SourceKind::Png => png::load(name, path, srgb, no_mips),
        SourceKind::Bmp => bmp::load(name, path, srgb, no_mips),
        SourceKind::Jpg => jpg::load(name, path, srgb, no_mips),
        SourceKind::Jpeg => jpeg::load(name, path, srgb, no_mips),
        SourceKind::Dds => dds::load(name, path, srgb, no_mips),
        SourceKind::Tga => tga::load(name, path, srgb, no_mips),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn supported_extensions_are_explicit() {
        for ext in ["png", "bmp", "jpg", "jpeg", "dds", "tga"] {
            assert!(
                SourceKind::from_path(Path::new(&format!("texture.{ext}"))).is_ok(),
                "missing {ext}"
            );
        }
    }

    #[test]
    fn xsr_and_exr_are_not_texture_sources() {
        assert!(SourceKind::from_path(Path::new("texture.xsr")).is_err());
        assert!(SourceKind::from_path(Path::new("texture.exr")).is_err());
    }
}

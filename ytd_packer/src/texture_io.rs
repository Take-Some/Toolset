use newengine_texture_container::TextureEncodedBuildEntry;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::texture_sources::{self, SourceKind};

pub fn collect_sources(
    input_dir: Option<&Path>,
    specs: &[String],
) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = specs
        .iter()
        .map(|spec| parse_texture_spec(spec))
        .collect::<Vec<_>>();
    if let Some(dir) = input_dir {
        collect_sources_recursive(dir, &mut out)?;
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    Ok(out)
}

pub fn load_texture_entry(
    name: String,
    path: PathBuf,
    srgb: bool,
    no_mips: bool,
) -> Result<TextureEncodedBuildEntry, String> {
    let kind = SourceKind::from_path(&path)?;
    texture_sources::load(kind, name, &path, srgb, no_mips)
}

pub fn normalize_logical_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

pub fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn collect_sources_recursive(dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|e| format!("read_dir '{}' failed: {e}", dir.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect_sources_recursive(&path, out)?;
        } else if path.is_file() && SourceKind::from_path(&path).is_ok() {
            let name = path
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or("texture")
                .to_owned();
            out.push((name, path));
        }
    }
    Ok(())
}

fn parse_texture_spec(spec: &str) -> (String, PathBuf) {
    if let Some((name, path)) = spec.split_once('=') {
        (name.to_owned(), PathBuf::from(path))
    } else {
        let path = PathBuf::from(spec);
        let name = path
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or("texture")
            .to_owned();
        (name, path)
    }
}

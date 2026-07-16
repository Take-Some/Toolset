use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FontDictionary {
    pub entries: Vec<FontEntry>,
}

#[derive(Debug, Clone)]
pub struct FontEntry {
    pub name: String,
    pub source_path: String,
    pub family: String,
    pub style: String,
    pub weight: u16,
    pub kind: FontKind,
    pub bytes: Vec<u8>,
    pub hash: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    Ttf,
    Otf,
    Woff,
    Woff2,
    Ttc,
}

impl FontKind {
    pub fn label(self) -> &'static str {
        match self {
            FontKind::Ttf => "ttf",
            FontKind::Otf => "otf",
            FontKind::Woff => "woff",
            FontKind::Woff2 => "woff2",
            FontKind::Ttc => "ttc",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        match &bytes[0..4] {
            b"\x00\x01\x00\x00" => Some(FontKind::Ttf),
            b"OTTO" => Some(FontKind::Otf),
            b"wOFF" => Some(FontKind::Woff),
            b"wOF2" => Some(FontKind::Woff2),
            b"ttcf" => Some(FontKind::Ttc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub entry: Option<String>,
    pub family: Option<String>,
    pub style: Option<String>,
    pub weight: Option<u16>,
}

pub fn import_sources(paths: &[PathBuf], opts: &ImportOptions) -> Result<FontDictionary, String> {
    if paths.len() > 1 && opts.entry.is_some() {
        return Err("--entry can only be used with a single source font".to_owned());
    }
    let mut expanded = Vec::new();
    for path in paths {
        if path.is_dir() {
            collect_fonts(path, &mut expanded)?;
        } else {
            expanded.push(path.clone());
        }
    }
    expanded.sort();
    expanded.dedup();
    if expanded.is_empty() {
        return Err("no font sources found".to_owned());
    }
    let mut entries = Vec::new();
    for path in expanded {
        entries.push(import_one(&path, opts)?);
    }
    Ok(FontDictionary { entries })
}

fn collect_fonts(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|e| format!("read_dir '{}' failed: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_fonts(&path, out)?;
        } else if is_font_source_path(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn import_one(path: &Path, opts: &ImportOptions) -> Result<FontEntry, String> {
    let bytes =
        fs::read(path).map_err(|e| format!("read font '{}' failed: {e}", path.display()))?;
    let kind = FontKind::from_bytes(&bytes)
        .ok_or_else(|| format!("unsupported or invalid font signature '{}'", path.display()))?;
    let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or("font");
    let name = sanitize_entry_name(opts.entry.as_deref().unwrap_or(stem));
    let family = opts.family.clone().unwrap_or_else(|| sanitize_family(stem));
    let style = opts.style.clone().unwrap_or_else(|| infer_style(stem));
    let weight = opts.weight.unwrap_or_else(|| infer_weight(stem));
    if !(1..=1000).contains(&weight) {
        return Err(format!(
            "font '{}' has invalid weight {}; expected 1..1000",
            path.display(),
            weight
        ));
    }
    Ok(FontEntry {
        name,
        source_path: path.to_string_lossy().replace('\\', "/"),
        family,
        style,
        weight,
        kind,
        hash: *blake3::hash(&bytes).as_bytes(),
        bytes,
    })
}

pub fn is_font_source_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|x| x.to_str())
            .map(|x| x.to_ascii_lowercase())
            .as_deref(),
        Some("ttf" | "otf" | "woff" | "woff2" | "ttc")
    )
}

pub fn sanitize_entry_name(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if out.chars().last() != Some('_') || mapped != '_' {
            out.push(mapped);
        }
    }
    let out = out.trim_matches('_').to_owned();
    if out.is_empty() {
        "font".to_owned()
    } else {
        out
    }
}

fn sanitize_family(value: &str) -> String {
    value
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn infer_style(stem: &str) -> String {
    let lower = stem.to_ascii_lowercase();
    if lower.contains("italic") {
        "Italic".to_owned()
    } else if lower.contains("bold") {
        "Bold".to_owned()
    } else {
        "Regular".to_owned()
    }
}

fn infer_weight(stem: &str) -> u16 {
    let lower = stem.to_ascii_lowercase();
    if lower.contains("thin") {
        100
    } else if lower.contains("light") {
        300
    } else if lower.contains("medium") {
        500
    } else if lower.contains("semibold") || lower.contains("semi-bold") {
        600
    } else if lower.contains("bold") {
        700
    } else if lower.contains("black") {
        900
    } else {
        400
    }
}

pub fn stable_hash64(value: &str) -> u64 {
    u64::from_le_bytes(
        blake3::hash(value.to_ascii_lowercase().as_bytes()).as_bytes()[0..8]
            .try_into()
            .unwrap(),
    )
}

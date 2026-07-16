use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn collect_sources(root: &Path) -> Result<Vec<PathBuf>, String> {
    collect(root, is_source)
}
pub fn collect_ymaps(root: &Path) -> Result<Vec<PathBuf>, String> {
    collect(root, is_ymap)
}

fn collect(root: &Path, accept: fn(&Path) -> bool) -> Result<Vec<PathBuf>, String> {
    if !root.exists() {
        return Err(format!("root does not exist '{}'", root.display()));
    }
    let mut out = Vec::new();
    walk(root, accept, &mut out)?;
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(path: &Path, accept: fn(&Path) -> bool, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if accept(path) {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in
        fs::read_dir(path).map_err(|e| format!("read_dir '{}' failed: {e}", path.display()))?
    {
        let entry =
            entry.map_err(|e| format!("read_dir entry '{}' failed: {e}", path.display()))?;
        let p = entry.path();
        if p.is_dir() {
            if !skip_dir(&p) {
                walk(&p, accept, out)?;
            }
        } else if accept(&p) {
            out.push(p);
        }
    }
    Ok(())
}

pub fn is_source(path: &Path) -> bool {
    let value = normalize(&path.to_string_lossy()).to_ascii_lowercase();
    value.ends_with(".ymap.xml") || value.ends_with(".ymap.json")
}

pub fn is_ymap(path: &Path) -> bool {
    normalize(&path.to_string_lossy())
        .to_ascii_lowercase()
        .ends_with(".ymap")
}

pub fn output_for_source(source: &Path) -> Result<PathBuf, String> {
    let value = source.to_string_lossy().replace('\\', "/");
    for suffix in [".ymap.xml", ".ymap.json"] {
        if let Some(base) = value.strip_suffix(suffix) {
            return Ok(PathBuf::from(format!("{base}.ymap")));
        }
    }
    Err(format!(
        "source '{}' must end with .ymap.xml or .ymap.json",
        source.display()
    ))
}

pub fn infer_logical_path(root: &Path, output: &Path) -> String {
    let rel = output
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| output.to_path_buf());
    let normalized = normalize(&rel.to_string_lossy());
    normalized
        .strip_prefix("gameAssets/")
        .unwrap_or(&normalized)
        .to_owned()
}

pub fn normalize(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    out = out.trim_start_matches('/').to_owned();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}

fn skip_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|it| it.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(name.as_str(), "target" | ".git" | "cache")
}

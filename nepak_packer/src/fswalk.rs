use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub disk_path: PathBuf,
    pub package_path: String,
}

pub fn collect_sources(input: &Path) -> Result<Vec<SourceFile>, String> {
    if input.is_file() {
        let name = input
            .file_name()
            .and_then(|x| x.to_str())
            .ok_or_else(|| format!("invalid file name '{}'", input.display()))?;
        let package_path = normalize_package_path(name);
        validate_package_path(&package_path)?;
        return Ok(vec![SourceFile { disk_path: input.to_path_buf(), package_path }]);
    }
    if !input.is_dir() {
        return Err(format!("input '{}' is neither file nor directory", input.display()));
    }
    let mut out = Vec::new();
    walk_dir(input, input, &mut out)?;
    out.sort_by(|a, b| a.package_path.cmp(&b.package_path));
    Ok(out)
}

fn walk_dir(root: &Path, dir: &Path, out: &mut Vec<SourceFile>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("read_dir '{}' failed: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            walk_dir(root, &path, out)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let package_path = normalize_package_path(&rel.to_string_lossy());
            validate_package_path(&package_path)?;
            out.push(SourceFile { disk_path: path, package_path });
        }
    }
    Ok(())
}

pub fn normalize_package_path(value: &str) -> String {
    let mut out = value.replace('\\', "/");
    while out.starts_with("./") {
        out = out[2..].to_owned();
    }
    out = out.trim_start_matches('/').to_owned();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}

pub fn validate_package_path(package_path: &str) -> Result<(), String> {
    let normalized = normalize_package_path(package_path);
    if normalized.is_empty() {
        return Err("unsafe package path: empty".to_owned());
    }
    if normalized != package_path {
        return Err(format!("unsafe package path '{package_path}': path must already be normalized"));
    }
    if normalized.starts_with('/') || normalized.starts_with('\\') {
        return Err(format!("unsafe package path '{package_path}': absolute path"));
    }
    if normalized.contains(':') {
        return Err(format!("unsafe package path '{package_path}': drive or scheme separator"));
    }
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("unsafe package path '{package_path}': invalid segment '{segment}'"));
        }
        if is_reserved_windows_name(segment) {
            return Err(format!("unsafe package path '{package_path}': reserved name '{segment}'"));
        }
    }
    Ok(())
}

pub fn safe_output_path(root: &Path, package_path: &str) -> Result<PathBuf, String> {
    validate_package_path(package_path)?;
    let normalized = normalize_package_path(package_path);
    let mut out = PathBuf::from(root);
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => return Err(format!("unsafe package path '{package_path}'")),
        }
    }
    Ok(out)
}

fn is_reserved_windows_name(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment).to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    )
}

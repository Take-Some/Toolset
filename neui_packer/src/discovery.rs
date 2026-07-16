use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn read_xml_or_nef8(input: &Path) -> Result<String, String> {
    let bytes = fs::read(input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    if bytes.get(0..4) == Some(b"NEF8") {
        northstar_neui_packer::decode_nef8_xmlcentral(&bytes)
    } else {
        String::from_utf8(bytes)
            .map_err(|e| format!("'{}' is neither NEF8 nor UTF-8 XML: {e}", input.display()))
    }
}

pub fn write_or_print_json(output: Option<&Path>, value: &serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n";
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(path, text.as_bytes())
            .map_err(|e| format!("write '{}' failed: {e}", path.display()))?;
        northstar_cli::ansi::ok(format!("wrote: {}", path.display()));
    } else {
        print!("{text}");
    }
    Ok(())
}

pub fn emit_or_write(
    root: &Path,
    source: &Path,
    target: &Path,
    bytes: &[u8],
    check: bool,
) -> Result<(), String> {
    if check {
        println!(
            "[CHECK] compiled: {} -> {} bytes={}",
            rel(root, source),
            rel(root, target),
            bytes.len()
        );
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create parent '{}' failed: {e}", parent.display()))?;
    }
    fs::write(target, bytes).map_err(|e| format!("write '{}' failed: {e}", target.display()))?;
    northstar_cli::ansi::ok(format!(
        "compiled: {} -> {} bytes={}",
        rel(root, source),
        rel(root, target),
        bytes.len()
    ));
    Ok(())
}

pub fn discover_xml_sources(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(&ui_asset_root(root), &mut out, ".neui.xml")?;
    Ok(out)
}

pub fn discover_neui_assets(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(&ui_asset_root(root), &mut out, ".neui")?;
    out.retain(|path| !path.to_string_lossy().ends_with(".neui.xml"));
    Ok(out)
}

pub fn target_path_for_xml(root: &Path, source: &Path) -> PathBuf {
    let name = source
        .file_name()
        .and_then(|it| it.to_str())
        .unwrap_or("generated.neui.xml");
    let target_name = name
        .strip_suffix(".neui.xml")
        .map(|stem| format!("{stem}.neui"))
        .unwrap_or_else(|| "generated.neui".to_owned());
    let replaced = source.with_file_name(&target_name);
    let asset_root = ui_asset_root(root);
    if let Ok(rel) = replaced.strip_prefix(&asset_root) {
        let mut out = PathBuf::new();
        for part in rel.components() {
            if part.as_os_str() != "src" {
                out.push(part.as_os_str());
            }
        }
        return asset_root.join(out);
    }
    replaced
        .strip_prefix(root)
        .map(|p| root.join(p))
        .unwrap_or_else(|_| asset_root.join(target_name))
}
pub fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub fn logical_asset_path_for_output(root: &Path, output: &Path) -> String {
    let normalized = output.to_string_lossy().replace('\\', "/");
    for marker in ["EngineRepo/NewEngine/neocore2/", "NewEngine/neocore2/"] {
        if let Some(idx) = normalized.find(marker) {
            return normalized[idx + marker.len()..]
                .trim_start_matches('/')
                .to_owned();
        }
    }
    rel(root, output)
}

pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn count_tag(xml: &str, name: &str) -> usize {
    let needle = format!("<{name}");
    let mut count = 0usize;
    let mut search = 0usize;
    while let Some(pos_rel) = xml[search..].find(&needle) {
        let pos = search + pos_rel;
        let next = xml.as_bytes().get(pos + needle.len()).copied();
        if matches!(
            next,
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')
        ) {
            count += 1;
        }
        search = pos + needle.len();
    }
    count
}

fn visit(path: &Path, out: &mut Vec<PathBuf>, suffix: &str) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    for entry in
        fs::read_dir(path).map_err(|e| format!("read_dir '{}' failed: {e}", path.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            visit(&path, out, suffix)?;
        } else if path
            .file_name()
            .and_then(|v| v.to_str())
            .map(|n| n.ends_with(suffix))
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
    Ok(())
}

fn ui_asset_root(root: &Path) -> PathBuf {
    let nested = root.join("EngineRepo/NewEngine/neocore2/assets/ui");
    if nested.exists() {
        nested
    } else {
        root.join("NewEngine/neocore2/assets/ui")
    }
}

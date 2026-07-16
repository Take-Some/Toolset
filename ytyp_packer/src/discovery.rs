use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn read_xml_or_ytyp(input: &Path) -> Result<String, String> {
    let bytes = fs::read(input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    if bytes.get(0..4) == Some(b"NEF8") {
        return Err(format!(
            "'{}' is legacy NEF8 metadata; .ytyp is now raw Y-Type Properties XML",
            input.display()
        ));
    }
    String::from_utf8(bytes).map_err(|e| {
        format!(
            "'{}' is not UTF-8 Y-Type Properties XML: {e}",
            input.display()
        )
    })
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
            "[CHECK] compiled properties: {} -> {} bytes={}",
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
        "compiled properties: {} -> {} bytes={}",
        rel(root, source),
        rel(root, target),
        bytes.len()
    ));
    Ok(())
}

pub fn discover_xml_sources(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(&asset_root(root), &mut out, ".ytyp.xml")?;
    Ok(out)
}

pub fn discover_ytyp_assets(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    visit(&asset_root(root), &mut out, ".ytyp")?;
    out.retain(|path| !path.to_string_lossy().ends_with(".ytyp.xml"));
    Ok(out)
}

pub fn target_path_for_xml(root: &Path, source: &Path) -> PathBuf {
    let name = source
        .file_name()
        .and_then(|it| it.to_str())
        .unwrap_or("generated.ytyp.xml");
    let target_name = name
        .strip_suffix(".ytyp.xml")
        .map(|stem| format!("{stem}.ytyp"))
        .unwrap_or_else(|| "generated.ytyp".to_owned());
    let replaced = source.with_file_name(&target_name);
    let source_parts = replaced
        .components()
        .map(|part| part.as_os_str().to_owned())
        .collect::<Vec<_>>();
    if source_parts.iter().any(|part| part == "src") {
        let mut out = PathBuf::new();
        for part in source_parts {
            if part != "src" {
                out.push(part);
            }
        }
        return out;
    }
    replaced
        .strip_prefix(root)
        .map(|p| root.join(p))
        .unwrap_or(replaced)
}

pub fn absolutize(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() || path.starts_with(root) {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub fn logical_asset_path_for_output(root: &Path, output: &Path) -> String {
    rel(root, output)
}

pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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

fn asset_root(root: &Path) -> PathBuf {
    let game_assets = root.join("gameAssets");
    if game_assets.exists() {
        return game_assets;
    }
    let neocore_assets = root.join("NewEngine/neocore2/assets");
    if neocore_assets.exists() {
        return neocore_assets;
    }
    let nested_neocore_assets = root.join("EngineRepo/NewEngine/neocore2/assets");
    if nested_neocore_assets.exists() {
        return nested_neocore_assets;
    }
    root.to_path_buf()
}

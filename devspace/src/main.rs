use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const LEGACY_TOOL_PATHS: &[&str] = &[
    "tools/netexturetool",
    "tools/NePak",
    "tools/nepak",
    "tools/nematerialtool",
    "tools/nelistfile",
    "tools/nelisyfile",
    "tools/DDSCubemap",
    "tools/DDSHeaderViewer",
    "tools/NoiseGenerator",
    "tools/AssetAnalysisTool",
    "tools/common",
    "tools/reference/DDSCubemap",
    "tools/reference/DDSHeaderViewer",
    "tools/reference/NoiseGenerator",
    "tools/reference/AssetAnalysisTool",
    "tools/quarantine/DDSCubemap",
    "tools/quarantine/DDSHeaderViewer",
    "tools/quarantine/NoiseGenerator",
    "tools/quarantine/AssetAnalysisTool",
];

const LEGACY_TOOL_IDENTITIES: &[&str] = &[
    "netexturetool",
    "nepak",
    "nematerialtool",
    "nelistfile",
    "nelisyfile",
    "ddscubemap",
    "ddsheaderviewer",
    "noisegenerator",
    "assetanalysistool",
];

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_help();
        return;
    }
    let command = args.remove(0);
    let result = match command.as_str() {
        "doctor" => cmd_doctor(parse_root(&args).unwrap_or_else(current_dir)),
        "validate-source" => cmd_validate_source(parse_root(&args).unwrap_or_else(current_dir)),
        "validate-registry" => cmd_validate_registry(parse_root(&args).unwrap_or_else(current_dir)),
        "asset-scan" => {
            let root = args.first().map(PathBuf::from).unwrap_or_else(current_dir);
            cmd_asset_scan(root)
        }
        "inspect-dds" => match args.first() {
            Some(path) => cmd_inspect_dds(PathBuf::from(path)),
            None => Err("inspect-dds requires a file path".into()),
        },
        "cubemap-layout" => cmd_cubemap_layout(),
        "noise-smoke" => cmd_noise_smoke(args),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    };
    if let Err(err) = result {
        eprintln!("[ERROR] {err}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!("North Star DEV Space native CLI");
    println!();
    println!("Usage:");
    println!("  northstar-devspace doctor [--root <repo>]");
    println!("  northstar-devspace validate-source [--root <repo>]");
    println!("  northstar-devspace validate-registry [--root <repo>]");
    println!("  northstar-devspace asset-scan <path>");
    println!("  northstar-devspace inspect-dds <file.dds>");
    println!("  northstar-devspace cubemap-layout");
    println!("  northstar-devspace noise-smoke --out <file.pgm> [--size N] [--seed N]");
}

fn current_dir() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn parse_root(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == "--root")
        .map(|pair| PathBuf::from(&pair[1]))
}

fn cmd_doctor(root: PathBuf) -> Result<(), String> {
    println!("[INFO] North Star DEV Space doctor root={}", root.display());
    cmd_validate_source(root.clone())?;
    cmd_validate_registry(root)?;
    Ok(())
}

fn cmd_validate_source(root: PathBuf) -> Result<(), String> {
    let mut failed = false;
    for rel in LEGACY_TOOL_PATHS {
        let path = root.join(rel);
        if path.exists() {
            eprintln!("[ERROR] legacy tool path is still present: {rel}");
            failed = true;
        }
    }
    if failed {
        Err("legacy tool paths must be deleted, not aliased".into())
    } else {
        println!("[OK] no legacy tool paths are present");
        Ok(())
    }
}

fn cmd_validate_registry(root: PathBuf) -> Result<(), String> {
    let tools = root.join("tools");
    let mut descriptors = Vec::new();
    collect_tool_descriptors(&tools, &mut descriptors).map_err(|e| e.to_string())?;
    if descriptors.is_empty() {
        return Err("no tool.json descriptors were found under tools/".into());
    }

    let mut ids: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut build_validators = 0_u32;
    let mut safe_for_build = 0_u32;
    let mut failed = false;

    for descriptor in descriptors.iter() {
        let text = fs::read_to_string(descriptor).map_err(|e| format!("{}: {e}", descriptor.display()))?;
        let id = json_string_field(&text, "id").ok_or_else(|| format!("tool descriptor misses id: {}", descriptor.display()))?;
        let kind = json_string_field(&text, "kind").unwrap_or_else(|| "unknown".to_owned());
        let normalized_id = normalized_identity(&id);
        for legacy in LEGACY_TOOL_IDENTITIES {
            if normalized_id == *legacy || normalized_id.ends_with(legacy) {
                eprintln!("[ERROR] tool id resurrects legacy identity: {id}");
                failed = true;
            }
        }
        if ids.insert(id.clone(), descriptor.clone()).is_some() {
            eprintln!("[ERROR] duplicate tool id: {id}");
            failed = true;
        }
        if json_bool_field(&text, "build_validation") {
            build_validators += 1;
        }
        if json_bool_field(&text, "safe_for_build") {
            safe_for_build += 1;
        }
        if kind == "binary" && json_bool_field(&text, "safe_for_build") {
            eprintln!("[ERROR] binary-only tool cannot be safe_for_build: {id}");
            failed = true;
        }
        println!("[TOOL] id={id} kind={kind} descriptor={}", descriptor.display());
    }

    if build_validators == 0 {
        eprintln!("[ERROR] no build_validation tool descriptor is registered");
        failed = true;
    }
    if failed {
        Err("tool registry is invalid".into())
    } else {
        println!("[OK] tool registry valid: {} descriptor(s), {safe_for_build} safe, {build_validators} validator(s)", ids.len());
        Ok(())
    }
}

fn normalized_identity(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let idx = text.find(&needle)?;
    let after_key = &text[idx + needle.len()..];
    let colon = after_key.find(':')?;
    let mut chars = after_key[colon + 1..].chars().peekable();
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for ch in chars {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn json_bool_field(text: &str, key: &str) -> bool {
    let needle = format!("\"{key}\"");
    let Some(idx) = text.find(&needle) else { return false; };
    let after_key = &text[idx + needle.len()..];
    let Some(colon) = after_key.find(':') else { return false; };
    after_key[colon + 1..].trim_start().starts_with("true")
}

fn cmd_asset_scan(root: PathBuf) -> Result<(), String> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut total = 0_u64;
    walk_files(&root, &mut |path| {
        total += 1;
        let key = path
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| format!(".{x}").to_lowercase())
            .unwrap_or_else(|| "<none>".into());
        *counts.entry(key).or_default() += 1;
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    println!("[INFO] scanned files: {total}");
    for (ext, count) in counts.iter().rev() {
        println!("[ASSET] {ext:16} {count}");
    }
    Ok(())
}

fn cmd_inspect_dds(path: PathBuf) -> Result<(), String> {
    let mut file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut data = [0_u8; 128];
    file.read_exact(&mut data)
        .map_err(|e| format!("failed to read DDS header: {e}"))?;
    if &data[0..4] != b"DDS " {
        return Err("bad DDS magic; expected 'DDS '".into());
    }
    let header_size = le_u32(&data, 4);
    let flags = le_u32(&data, 8);
    let height = le_u32(&data, 12);
    let width = le_u32(&data, 16);
    let pitch_or_linear = le_u32(&data, 20);
    let depth = le_u32(&data, 24);
    let mip_count = le_u32(&data, 28);
    let pf_flags = le_u32(&data, 80);
    let fourcc = &data[84..88];
    let fourcc_text = String::from_utf8_lossy(fourcc).trim_matches('\0').to_string();
    let rgb_bits = le_u32(&data, 88);
    println!("[DDS] file={}", path.display());
    println!("[DDS] header_size={header_size} flags=0x{flags:08X}");
    println!("[DDS] width={width} height={height} depth={depth} mips={mip_count}");
    println!("[DDS] pitch_or_linear_size={pitch_or_linear}");
    println!("[DDS] pixel_flags=0x{pf_flags:08X} fourcc={} rgb_bits={rgb_bits}", if fourcc_text.is_empty() { "<none>" } else { &fourcc_text });
    Ok(())
}

fn cmd_cubemap_layout() -> Result<(), String> {
    println!("[CUBEMAP] Native 4x3 cross layout contract");
    println!("[CUBEMAP]           +---------+");
    println!("[CUBEMAP]           | UP  -Z  |");
    println!("[CUBEMAP] +---------+---------+---------+---------+");
    println!("[CUBEMAP] | LF  +X  | DN  -Y  | RT  -X  | BK  +Y  |");
    println!("[CUBEMAP] +---------+---------+---------+---------+");
    println!("[CUBEMAP]           | FR  +Z  |");
    println!("[CUBEMAP]           +---------+");
    for face in cubemap_faces() {
        println!(
            "[FACE] index={} name={} direction={} rotation_deg={} cross_cell=({}, {})",
            face.index, face.name, face.direction, face.rotation_deg, face.cell_x, face.cell_y
        );
    }
    Ok(())
}

struct CubemapFace {
    index: u8,
    name: &'static str,
    direction: &'static str,
    rotation_deg: u16,
    cell_x: u8,
    cell_y: u8,
}

fn cubemap_faces() -> [CubemapFace; 6] {
    [
        CubemapFace { index: 0, name: "LF", direction: "+X", rotation_deg: 270, cell_x: 0, cell_y: 1 },
        CubemapFace { index: 1, name: "RT", direction: "-X", rotation_deg: 90,  cell_x: 2, cell_y: 1 },
        CubemapFace { index: 2, name: "BK", direction: "+Y", rotation_deg: 0,   cell_x: 3, cell_y: 1 },
        CubemapFace { index: 3, name: "DN", direction: "-Y", rotation_deg: 180, cell_x: 1, cell_y: 1 },
        CubemapFace { index: 4, name: "FR", direction: "+Z", rotation_deg: 180, cell_x: 1, cell_y: 2 },
        CubemapFace { index: 5, name: "UP", direction: "-Z", rotation_deg: 0,   cell_x: 1, cell_y: 0 },
    ]
}

fn cmd_noise_smoke(args: Vec<String>) -> Result<(), String> {
    let out = value_after(&args, "--out").ok_or("noise-smoke requires --out <file.pgm>")?;
    let size = value_after(&args, "--size")
        .and_then(|x| x.parse::<usize>().ok())
        .unwrap_or(128)
        .clamp(4, 4096);
    let seed = value_after(&args, "--seed")
        .and_then(|x| x.parse::<u32>().ok())
        .unwrap_or(0x4E535452);
    let out_path = PathBuf::from(out);
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let mut file = File::create(&out_path).map_err(|e| e.to_string())?;
    writeln!(file, "P5\n{size} {size}\n255").map_err(|e| e.to_string())?;
    for y in 0..size {
        for x in 0..size {
            let value = hash_noise(x as u32, y as u32, seed);
            file.write_all(&[value]).map_err(|e| e.to_string())?;
        }
    }
    println!("[OK] wrote deterministic smoke-noise PGM: {}", out_path.display());
    Ok(())
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn hash_noise(x: u32, y: u32, seed: u32) -> u8 {
    let mut v = seed ^ x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    v ^= v >> 16;
    v = v.wrapping_mul(0x7FEB_352D);
    v ^= v >> 15;
    v = v.wrapping_mul(0x846C_A68B);
    v ^= v >> 16;
    (v & 0xFF) as u8
}

fn le_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

fn collect_tool_descriptors(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "target" || name == ".git" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_tool_descriptors(&path, out)?;
        } else if path.file_name().and_then(|x| x.to_str()) == Some("tool.json") {
            out.push(path);
        }
    }
    Ok(())
}

fn walk_files<F>(root: &Path, f: &mut F) -> io::Result<()>
where
    F: FnMut(&Path) -> io::Result<()>,
{
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), ".git" | ".takesome" | ".northstar" | "target" | "node_modules" | "logs" | "cache") {
                continue;
            }
            walk_files(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}

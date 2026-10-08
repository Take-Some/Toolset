use crate::args::{parse_args, required_input, required_output};
use crate::{binary, help, manifest, nef8};
use serde_json::json;
use std::fs;
use std::path::Path;

const TOOL_NAME: &str = "northstar-yscd-packer";

pub fn dispatch(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        help::print_help();
        return Ok(());
    }
    match raw_args[0].as_str() {
        "pack" | "build" | "create" => run_pack(&raw_args[1..]),
        "unpack" | "extract" => run_unpack(&raw_args[1..]),
        "inspect" | "parse" => run_inspect(&raw_args[1..]),
        "verify" | "validate" => run_verify(&raw_args[1..]),
        "list" => run_list(&raw_args[1..]),
        "accepted-inputs" | "--accepted-inputs" | "inputs" | "formats" => {
            northstar_cli::ansi::info(format!("{TOOL_NAME} version={}", env!("CARGO_PKG_VERSION")));
            northstar_cli::ansi::info("accepted input files: *.json YSCD manifests; *.wav, *.ogg, *.opus, *.flac, *.mp3 embedded audio; *.yscd for read commands");
            northstar_cli::ansi::info("produced output files: *.yscd NEF8 sound cue dictionaries; unpacked manifest.json and encoded audio payloads");
            Ok(())
        }
        "doctor" | "--doctor" => {
            northstar_cli::ansi::ok(format!("{TOOL_NAME} doctor passed"));
            northstar_cli::ansi::info(format!("version={}", env!("CARGO_PKG_VERSION")));
            northstar_cli::ansi::info(format!("nef8_content_kind={}", nef8::CONTENT_KIND_YSCD));
            northstar_cli::ansi::info(format!("body_schema={}", binary::BODY_SCHEMA_VERSION));
            Ok(())
        }
        "version" | "--version" | "-V" => {
            northstar_cli::ansi::info(format!("{TOOL_NAME} {}", env!("CARGO_PKG_VERSION")));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            help::print_help();
            Ok(())
        }
        other => Err(format!("unknown command '{other}'. Use --help.")),
    }
}

fn run_pack(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "pack", "manifest.json")?;
    let output = required_output(&cfg, "pack", "file.yscd")?;
    let manifest = manifest::load_manifest(&input)?;
    let source_root = cfg
        .source_root
        .clone()
        .or_else(|| input.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| Path::new(".").to_path_buf());
    if cfg.debug {
        northstar_cli::ansi::info(format!("manifest={}", input.display()));
        northstar_cli::ansi::info(format!("source_root={}", source_root.display()));
        northstar_cli::ansi::info(format!("output={}", output.display()));
    }
    let body = binary::build_body(&manifest, &source_root)?;
    let packed = nef8::pack_yscd(&body, manifest.cues.len())?;
    write_bytes(&output, &packed, cfg.overwrite)?;
    let clip_count = manifest.cues.iter().map(|c| c.clips.len()).sum::<usize>();
    northstar_cli::ansi::ok(format!(
        "built YSCD: {} cues={} clips={} nef8_bytes={} body_bytes={}",
        output.display(),
        manifest.cues.len(),
        clip_count,
        packed.len(),
        body.len()
    ));
    Ok(())
}

fn run_unpack(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "unpack", "file.yscd")?;
    let output = required_output(&cfg, "unpack", "directory")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let (_, body) = nef8::decode_body(&bytes)?;
    let decoded = binary::parse_body(&body)?;
    if let Some(entry) = cfg.entry.as_deref() {
        return unpack_single_cue(&decoded, entry, &output, cfg.overwrite);
    }
    binary::write_unpacked(&decoded, &output, cfg.overwrite)?;
    northstar_cli::ansi::ok(format!(
        "unpacked YSCD: {} -> {} cues={} clips={}",
        input.display(),
        output.display(),
        decoded.manifest.cues.len(),
        decoded.payloads.len()
    ));
    Ok(())
}

fn unpack_single_cue(
    decoded: &binary::DecodedDictionary,
    entry: &str,
    output: &Path,
    overwrite: bool,
) -> Result<(), String> {
    let cue = decoded
        .manifest
        .cues
        .iter()
        .find(|cue| cue.name.eq_ignore_ascii_case(entry))
        .cloned()
        .ok_or_else(|| format!("YSCD cue '{entry}' not found"))?;
    let payloads = decoded
        .payloads
        .iter()
        .filter(|p| p.cue.eq_ignore_ascii_case(entry))
        .cloned()
        .collect::<Vec<_>>();
    let single = binary::DecodedDictionary {
        manifest: manifest::DictionaryManifest {
            cues: vec![cue],
            ..manifest::DictionaryManifest::default()
        },
        payloads,
    };
    binary::write_unpacked(&single, output, overwrite)?;
    northstar_cli::ansi::ok(format!("unpacked cue '{entry}' -> {}", output.display()));
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "inspect", "file.yscd")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let (header, body) = nef8::decode_body(&bytes)?;
    let decoded = binary::parse_body(&body)?;
    let value = json!({
        "schema": "northstar.yscd.nef8.inspect.v1",
        "ok": true,
        "file": input.to_string_lossy(),
        "container": "newengine.listfile.nef8.yscd",
        "header": {
            "magic": "NEF8",
            "content_kind": header.content_kind,
            "content_schema_version": header.content_schema_version,
            "entry_count": header.entry_count,
            "body_len": header.body_len,
            "body_uncompressed_len": header.body_uncompressed_len,
        },
        "dictionary": binary::inspect_json(&decoded),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn run_verify(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "verify", "file.yscd")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let (header, body) = nef8::decode_body(&bytes)?;
    let decoded = binary::parse_body(&body)?;
    if header.entry_count as usize != decoded.manifest.cues.len() {
        return Err(format!(
            "NEF8 entry_count={} differs from YSCD cue_count={}",
            header.entry_count,
            decoded.manifest.cues.len()
        ));
    }
    northstar_cli::ansi::ok(format!(
        "verified YSCD: {} cues={} clips={} embedded_bytes={}",
        input.display(),
        decoded.manifest.cues.len(),
        decoded.payloads.len(),
        decoded
            .payloads
            .iter()
            .map(|p| p.bytes.len())
            .sum::<usize>()
    ));
    Ok(())
}

fn run_list(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "list", "file.yscd")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let (_, body) = nef8::decode_body(&bytes)?;
    let decoded = binary::parse_body(&body)?;
    let logical = input.to_string_lossy().replace('\\', "/");
    for cue in decoded.manifest.cues {
        println!("{logical}@{}", cue.name);
    }
    Ok(())
}

fn write_bytes(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), String> {
    if path.exists() && !overwrite {
        return Err(format!(
            "output '{}' exists; use --overwrite",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create '{}' failed: {e}", parent.display()))?;
        }
    }
    fs::write(path, bytes).map_err(|e| format!("write '{}' failed: {e}", path.display()))
}

use crate::diagnostics;
use newengine_texture_container::{
    pack_encoded_with_options, parse, parse_manifest_only, write_dds_runtime_mip_chain,
    TextureBuildOptions,
};
use serde_json::json;
use std::fs;

use crate::{
    args::{parse_args, required_input, required_output},
    fixture_gen, help, nef8, texture_io,
};

const TOOL_NAME: &str = "northstar-ytd-packer";
const ACCEPTED_INPUTS: &str = "*.png, *.bmp, *.jpg, *.jpeg, *.dds, *.tga texture sources; *.ytd for inspect/validate/extract/dump-netd";
const PRODUCED_OUTPUTS: &str =
    "*.ytd runtime NEF8 texture dictionary; extracted *.dds files; *.netd body dumps";

pub fn dispatch(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        help::print_help();
        help::wait_for_enter();
        return Ok(());
    }
    if raw_args
        .iter()
        .skip(1)
        .any(|it| it == "--help" || it == "-h")
    {
        help::print_help();
        help::wait_for_enter();
        return Ok(());
    }
    match raw_args[0].as_str() {
        "pack" | "build" | "create" => run_pack(&raw_args[1..]),
        "inspect" | "parse" => run_inspect(&raw_args[1..]),
        "validate" => run_validate(&raw_args[1..]),
        "extract" => run_extract(&raw_args[1..]),
        "manifest" => run_inspect(&raw_args[1..]),
        "dump-netd" => run_dump_netd(&raw_args[1..]),
        "write-smoke-fixtures" => run_write_smoke_fixtures(&raw_args[1..]),
        "accepted-inputs" | "--accepted-inputs" | "inputs" | "formats" => {
            diagnostics::print_contract(TOOL_NAME, ACCEPTED_INPUTS, PRODUCED_OUTPUTS);
            Ok(())
        }
        "doctor" | "--doctor" => {
            diagnostics::print_doctor_ok(TOOL_NAME);
            Ok(())
        }
        "version" | "--version" | "-V" => {
            diagnostics::print_version(TOOL_NAME);
            Ok(())
        }
        "help" | "--help" | "-h" => {
            help::print_help();
            help::wait_for_enter();
            Ok(())
        }
        other => Err(format!(
            "unknown command '{other}'. Use --help to list supported commands."
        )),
    }
}

fn run_write_smoke_fixtures(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let output = required_output(&cfg, "write-smoke-fixtures", "directory")?;
    fixture_gen::write_smoke_fixtures(&output)?;
    northstar_cli::ansi::ok(format!("wrote YTD smoke fixtures: {}", output.display()));
    Ok(())
}

fn run_pack(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let output = required_output(&cfg, "pack", "file.ytd")?;
    let sources = texture_io::collect_sources(cfg.input_dir.as_deref(), &cfg.textures)?;
    diagnostics::print_operation(
        TOOL_NAME,
        "pack",
        cfg.debug,
        ACCEPTED_INPUTS,
        PRODUCED_OUTPUTS,
    );
    diagnostics::print_debug_value(cfg.debug, "source_count", sources.len());
    for (name, path) in &sources {
        diagnostics::print_debug_value(cfg.debug, "source", format!("{}={}", name, path.display()));
    }
    diagnostics::print_debug_value(cfg.debug, "output", output.display());
    if sources.is_empty() {
        return Err(
            "pack requires --texture name=path or --input-dir with PNG/BMP/JPG/JPEG/DDS/TGA files"
                .to_owned(),
        );
    }
    for requested in &cfg.linear_entries {
        if !sources
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(requested))
        {
            return Err(format!(
                "--linear-entry '{requested}' does not match any input texture entry"
            ));
        }
    }
    northstar_cli::ansi::info(format!("YTD build started textures={}", sources.len()));
    let mut entries = Vec::with_capacity(sources.len());
    for (name, path) in sources {
        let entry_srgb = cfg.srgb
            && !cfg
                .linear_entries
                .iter()
                .any(|entry| entry.eq_ignore_ascii_case(&name));
        entries.push(texture_io::load_texture_entry(
            name,
            path,
            entry_srgb,
            cfg.no_mips,
        )?);
    }
    let options = if cfg.raw_data {
        TextureBuildOptions::raw_runtime()
    } else {
        TextureBuildOptions::default()
    };
    let netd = pack_encoded_with_options(entries, options)
        .map_err(|e| format!("NETD pack failed: {e}"))?;
    let logical = texture_io::normalize_logical_path(&output.to_string_lossy());
    let ytd = nef8::pack_ytd(&netd, &logical, 1)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create parent '{}' failed: {e}", parent.display()))?;
    }
    fs::write(&output, ytd).map_err(|e| format!("write '{}' failed: {e}", output.display()))?;
    northstar_cli::ansi::ok(format!("built YTD: {}", output.display()));
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "inspect")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let header = nef8::parse_header(&bytes)?;
    let netd = nef8::decode_ytd_body(&bytes, &header)?;
    let manifest = parse_manifest_only(&netd).map_err(|e| format!("NETD parse failed: {e}"))?;
    let entries = manifest.entries.iter().map(|e| json!({
        "name": e.name,
        "entry_ref": format!("{}@{}", texture_io::normalize_logical_path(&input.to_string_lossy()), e.name),
        "width": e.width,
        "height": e.height,
        "format": e.format,
        "color_space": e.color_space,
        "mip_count": e.mip_count,
        "byte_len": e.byte_len,
    })).collect::<Vec<_>>();
    let value = json!({
        "schema": "northstar.ytd.inspect.v1",
        "ok": true,
        "file": input.to_string_lossy(),
        "container": "newengine.listfile.nef8.ytd",
        "payload": "NETD",
        "header": {
            "magic": "NEF8",
            "content_kind": header.content_kind,
            "entry_count": header.entry_count,
            "body_len": header.body_len,
            "body_uncompressed_len": header.body_uncompressed_len,
        },
        "texture_dictionary": {
            "version": manifest.version,
            "default_format": manifest.default_format,
            "entry_count": manifest.entries.len(),
            "entries": entries,
        }
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn run_validate(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "validate")?;
    let netd = read_netd(&input)?;
    let dictionary = parse(&netd).map_err(|e| format!("NETD validate failed: {e}"))?;
    northstar_cli::ansi::ok(format!(
        "validated YTD: {} entries={}",
        input.display(),
        dictionary.entries().len()
    ));
    Ok(())
}

fn run_dump_netd(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "dump-netd")?;
    let output = required_output(&cfg, "dump-netd", "file.netd")?;
    fs::write(&output, read_netd(&input)?)
        .map_err(|e| format!("write '{}' failed: {e}", output.display()))?;
    northstar_cli::ansi::ok(format!("dumped NETD: {}", output.display()));
    Ok(())
}

fn run_extract(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "extract")?;
    let output = required_output(&cfg, "extract", "directory")?;
    fs::create_dir_all(&output)
        .map_err(|e| format!("create output '{}' failed: {e}", output.display()))?;
    let netd = read_netd(&input)?;
    let dictionary = parse(&netd).map_err(|e| format!("NETD parse failed: {e}"))?;
    for meta in dictionary.entries() {
        if cfg
            .entry
            .as_deref()
            .is_some_and(|filter| !meta.name.eq_ignore_ascii_case(filter))
        {
            continue;
        }
        let entry = dictionary.entry(&meta.name).map_err(|e| e.to_string())?;
        let mips = meta
            .mips
            .iter()
            .map(|m| {
                let bytes = entry
                    .mip_bytes(m.level)
                    .ok_or_else(|| format!("missing mip {} for {}", m.level, meta.name))?;
                Ok(newengine_texture_container::TextureEncodedMipData {
                    level: m.level,
                    width: m.width,
                    height: m.height,
                    bytes: bytes.to_vec(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let dds = write_dds_runtime_mip_chain(meta.width, meta.height, &meta.format, &mips)
            .map_err(|e| format!("DDS export '{}' failed: {e}", meta.name))?;
        let path = output.join(format!(
            "{}.dds",
            texture_io::sanitize_file_name(&meta.name)
        ));
        fs::write(&path, dds).map_err(|e| format!("write '{}' failed: {e}", path.display()))?;
        northstar_cli::ansi::ok(format!("extracted: {}", path.display()));
    }
    northstar_cli::ansi::ok("extract completed textures={count}");
    Ok(())
}

fn read_netd(input: &std::path::Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let header = nef8::parse_header(&bytes)?;
    nef8::decode_ytd_body(&bytes, &header)
}

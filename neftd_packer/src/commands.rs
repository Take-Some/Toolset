use crate::diagnostics;
use std::fs;
use std::path::Path;

use crate::{
    args::{parse_args, required_input, required_output, required_sources},
    font::{self, ImportOptions},
    help, nef8,
};

const TOOL_NAME: &str = "northstar-neftd-packer";
const ACCEPTED_INPUTS: &str =
    "*.ttf, *.otf, *.ttc, *.woff, *.woff2 font sources; *.neftd for inspect/list/validate/extract";
const PRODUCED_OUTPUTS: &str = "*.neftd runtime NEF8 font dictionary; extracted *.fontbin payloads";

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
        "create" | "pack" | "build" => run_pack(&raw_args[1..]),
        "inspect" | "parse" => run_inspect(&raw_args[1..]),
        "validate" => run_validate(&raw_args[1..]),
        "doctor" | "--doctor" => {
            diagnostics::print_doctor_ok(TOOL_NAME);
            Ok(())
        }
        "list" => run_list(&raw_args[1..]),
        "extract" => run_extract(&raw_args[1..]),
        "accepted-inputs" | "--accepted-inputs" | "inputs" | "formats" => {
            diagnostics::print_contract(TOOL_NAME, ACCEPTED_INPUTS, PRODUCED_OUTPUTS);
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

fn run_pack(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let sources = required_sources(&cfg, "pack")?;
    let output = required_output(&cfg, "pack", "file.neftd")?;
    diagnostics::print_operation(
        TOOL_NAME,
        "pack",
        cfg.debug,
        ACCEPTED_INPUTS,
        PRODUCED_OUTPUTS,
    );
    diagnostics::print_debug_value(cfg.debug, "source_count", sources.len());
    for source in &sources {
        diagnostics::print_debug_value(cfg.debug, "source", source.display());
    }
    diagnostics::print_debug_value(cfg.debug, "output", output.display());
    let opts = ImportOptions {
        entry: cfg.entry.clone(),
        family: cfg.family.clone(),
        style: cfg.style.clone(),
        weight: cfg.weight,
    };
    let dict = font::import_sources(&sources, &opts)?;
    let logical = output.to_string_lossy().replace('\\', "/");
    let bytes = nef8::pack_neftd(&dict, &logical, !cfg.no_compress)?;
    write_bytes(&output, &bytes, true)?;
    northstar_cli::ansi::ok(format!(
        "built NEFTD font dictionary: {} entries={}",
        output.display(),
        dict.entries.len()
    ));
    for entry in &dict.entries {
        northstar_cli::ansi::ok(format!(
            "entry {}@{} kind={} family='{}' style='{}' weight={}",
            logical,
            entry.name,
            entry.kind.label(),
            entry.family,
            entry.style,
            entry.weight
        ));
    }
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "inspect")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let value = nef8::inspect_json(&bytes, &input.to_string_lossy())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn run_validate(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "validate")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let (_, entries) = nef8::parse_neftd(&bytes, &input.to_string_lossy())?;
    northstar_cli::ansi::ok(format!(
        "validated NEFTD font dictionary: {} entries={}",
        input.display(),
        entries.len()
    ));
    Ok(())
}

fn run_list(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "list")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let (_, entries) = nef8::parse_neftd(&bytes, &input.to_string_lossy())?;
    for entry in entries {
        println!("{}", entry.selector);
    }
    Ok(())
}

fn run_extract(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "extract")?;
    let out_dir = cfg
        .out_dir
        .clone()
        .or(cfg.output.clone())
        .ok_or("extract requires --out-dir <directory> or --output <directory>")?;
    let entry_name = cfg
        .entry
        .as_deref()
        .ok_or("extract requires --entry <font_entry>")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let (name, payload) = nef8::extract_entry(&bytes, &input.to_string_lossy(), entry_name)?;
    let out = out_dir.join(format!("{}.fontbin", name));
    write_bytes(&out, &payload, cfg.overwrite)?;
    northstar_cli::ansi::ok(format!(
        "extracted font entry: {} -> {} bytes={}",
        entry_name,
        out.display(),
        payload.len()
    ));
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
                .map_err(|e| format!("create parent '{}' failed: {e}", parent.display()))?;
        }
    }
    fs::write(path, bytes).map_err(|e| format!("write '{}' failed: {e}", path.display()))
}

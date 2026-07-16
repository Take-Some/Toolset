use std::fs;

use crate::diagnostics;

use crate::{
    args::{parse_args, required_compare, required_input, required_output},
    fswalk,
    help,
    nepak,
};

const TOOL_NAME: &str = "northstar-nepak-manager";
const ACCEPTED_INPUTS: &str = "directory trees and opaque loose files for pack; *.nepak for inspect/manifest/list/verify/extract/mount-test/diff";
const PRODUCED_OUTPUTS: &str = "*.nepak VFS container; extracted directory trees; clean JSON/list payloads";

pub fn dispatch(raw_args: Vec<String>) -> Result<(), String> {
    if raw_args.is_empty() {
        help::print_help();
        help::wait_for_enter();
        return Ok(());
    }
    if raw_args.iter().skip(1).any(|it| it == "--help" || it == "-h") {
        help::print_help();
        help::wait_for_enter();
        return Ok(());
    }
    match raw_args[0].as_str() {
        "pack" | "build" | "create" => run_pack(&raw_args[1..]),
        "extract" => run_extract(&raw_args[1..]),
        "inspect" => run_inspect(&raw_args[1..]),
        "manifest" => run_manifest(&raw_args[1..]),
        "list" => run_list(&raw_args[1..]),
        "verify" => run_verify(&raw_args[1..]),
        "mount-test" => run_mount_test(&raw_args[1..]),
        "diff" => run_diff(&raw_args[1..]),
        "doctor" | "--doctor" => {
            diagnostics::print_doctor_ok(TOOL_NAME);
            Ok(())
        }
        "accepted-inputs" | "--accepted-inputs" => {
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
        other => Err(format!("unknown command '{other}'. Use --help to list supported commands.")),
    }
}

fn run_pack(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "pack")?;
    let output = required_output(&cfg, "pack")?;
    let sources = fswalk::collect_sources(&input)?;
    diagnostics::print_operation(TOOL_NAME, "pack", cfg.debug, ACCEPTED_INPUTS, PRODUCED_OUTPUTS);
    diagnostics::print_debug_value(cfg.debug, "input", input.display());
    diagnostics::print_debug_value(cfg.debug, "output", output.display());
    diagnostics::print_debug_value(cfg.debug, "source_count", sources.len());
    nepak::pack_sources(&sources, &output, !cfg.no_compress)?;
    northstar_cli::ansi::ok(format!("built clean NEPAK package: {} entries={}", output.display(), sources.len()));
    Ok(())
}

fn run_extract(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "extract")?;
    let output = required_output(&cfg, "extract")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let count = nepak::extract_to(&bytes, &output, cfg.path.as_deref(), cfg.overwrite)?;
    northstar_cli::ansi::ok(format!("extracted clean NEPAK package: {} entries={} output={}", input.display(), count, output.display()));
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "inspect")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let value = nepak::inspect_json(&bytes, &input)?;
    println!("{}", serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?);
    Ok(())
}

fn run_manifest(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "manifest")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let value = nepak::manifest_json(&bytes)?;
    println!("{}", serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?);
    Ok(())
}

fn run_list(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "list")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    for path in nepak::list_paths(&bytes)? {
        println!("{path}");
    }
    Ok(())
}

fn run_verify(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "verify")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let count = nepak::validate_bytes(&bytes)?;
    northstar_cli::ansi::ok(format!("verified clean NEPAK package: {} entries={}", input.display(), count));
    Ok(())
}

fn run_mount_test(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "mount-test")?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let value = nepak::mount_test_json(&bytes, &input)?;
    println!("{}", serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?);
    Ok(())
}

fn run_diff(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let old_path = required_input(&cfg, "diff")?;
    let new_path = required_compare(&cfg, "diff")?;
    let old_bytes = fs::read(&old_path).map_err(|e| format!("read '{}' failed: {e}", old_path.display()))?;
    let new_bytes = fs::read(&new_path).map_err(|e| format!("read '{}' failed: {e}", new_path.display()))?;
    let value = nepak::diff_json(&old_bytes, &new_bytes)?;
    println!("{}", serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?);
    Ok(())
}

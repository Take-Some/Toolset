use crate::diagnostics;
use std::fs;

use crate::{
    args::{parse_args, required_input},
    discovery, help,
    properties::{compile_ytyp_xml_to_properties, inspect_ytyp_json},
    xmlmeta::{
        dependencies, entry_names, manifest_json_for_metadata, metadata_projection_json,
        validate_metadata_xml,
    },
};

const TOOL_NAME: &str = "northstar-ytyp-packer";
const ACCEPTED_INPUTS: &str = "*.ytyp.xml source Y-Type Properties XML files; *.ytyp canonical NEF8 V2 YTYP assets for inspect/validate/dump";
const PRODUCED_OUTPUTS: &str =
    "*.ytyp NEF8 runtime asset; XML dumps; JSON manifest/metadata/dependency projections";

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
        "compile" | "build" | "pack" => run_compile(&raw_args[1..]),
        "inspect" => run_inspect(&raw_args[1..]),
        "validate" => run_validate(&raw_args[1..]),
        "manifest" => run_manifest(&raw_args[1..]),
        "dump-xml" | "dump-xmlmetadata" => run_dump_xml(&raw_args[1..]),
        "dump-metadata" => run_dump_metadata(&raw_args[1..]),
        "dump-dependencies" => run_dump_dependencies(&raw_args[1..]),
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
        other if other.starts_with('-') => run_compile(&raw_args),
        other => Err(format!(
            "unknown command '{other}'. Use --help to list supported commands."
        )),
    }
}

fn run_compile(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let root = cfg.root;
    let mut xml_sources = Vec::new();
    if let Some(input) = cfg.input {
        xml_sources.push(input);
    }
    if cfg.all || xml_sources.is_empty() {
        xml_sources.extend(discovery::discover_xml_sources(&root)?);
    }
    xml_sources.sort();
    xml_sources.dedup();
    if xml_sources.is_empty() {
        northstar_cli::ansi::warn("no .ytyp.xml sources found");
        return Ok(());
    }

    northstar_cli::ansi::info("YTYP NEF8 build started");
    diagnostics::print_operation(
        TOOL_NAME,
        "compile",
        cfg.debug,
        ACCEPTED_INPUTS,
        PRODUCED_OUTPUTS,
    );
    diagnostics::print_debug_value(cfg.debug, "root", root.display());
    diagnostics::print_debug_value(cfg.debug, "source_count", xml_sources.len());
    for source in &xml_sources {
        diagnostics::print_debug_value(cfg.debug, "source", source.display());
    }
    for source in &xml_sources {
        let source_path = discovery::absolutize(&root, source);
        let target = cfg
            .output
            .clone()
            .unwrap_or_else(|| discovery::target_path_for_xml(&root, &source_path));
        let xml = fs::read_to_string(&source_path)
            .map_err(|e| format!("read '{}' failed: {e}", source_path.display()))?;
        for warning in validate_metadata_xml(&xml, &discovery::rel(&root, &source_path))? {
            northstar_cli::ansi::warn(format!(
                "{}: {}",
                discovery::rel(&root, &source_path),
                warning
            ));
        }
        let logical_path = cfg
            .logical_path
            .clone()
            .unwrap_or_else(|| discovery::logical_asset_path_for_output(&root, &target));
        let bytes = compile_ytyp_xml_to_properties(
            &xml,
            &logical_path,
            &discovery::rel(&root, &source_path),
            entry_names(&xml).len() as u64,
        )?;
        discovery::emit_or_write(&root, &source_path, &target, &bytes, cfg.check)?;
    }
    northstar_cli::ansi::ok(format!(
        "YTYP NEF8 build completed compiled={} check={}",
        xml_sources.len(),
        cfg.check
    ));
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let value = inspect_ytyp_json(&bytes)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn run_validate(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let root = cfg.root;
    let mut inputs = Vec::new();
    if let Some(input) = cfg.input {
        inputs.push(input);
    }
    if cfg.all || inputs.is_empty() {
        inputs.extend(discovery::discover_xml_sources(&root)?);
        inputs.extend(discovery::discover_ytyp_assets(&root)?);
    }
    inputs.sort();
    inputs.dedup();
    if inputs.is_empty() {
        northstar_cli::ansi::warn("no .ytyp.xml or .ytyp assets found");
        return Ok(());
    }
    let mut entry_count = 0usize;
    let mut dep_count = 0usize;
    for input in inputs {
        let path = discovery::absolutize(&root, &input);
        let xml = discovery::read_xml_or_ytyp(&path)?;
        for warning in validate_metadata_xml(&xml, &discovery::rel(&root, &path))? {
            northstar_cli::ansi::warn(format!("{}: {}", discovery::rel(&root, &path), warning));
        }
        entry_count += entry_names(&xml).len();
        dep_count += dependencies(&xml).len();
        northstar_cli::ansi::ok(format!("validated: {}", discovery::rel(&root, &path)));
    }
    northstar_cli::ansi::ok(format!(
        "validated canonical YTYP: entry(s)={entry_count}, dependency ref(s)={dep_count}"
    ));
    Ok(())
}

fn run_manifest(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = discovery::read_xml_or_ytyp(&input)?;
    let logical = cfg
        .logical_path
        .unwrap_or_else(|| discovery::logical_asset_path_for_output(&cfg.root, &input));
    discovery::write_or_print_json(
        cfg.output.as_deref(),
        &manifest_json_for_metadata(&xml, &logical)?,
    )
}

fn run_dump_xml(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = discovery::read_xml_or_ytyp(&input)?;
    if let Some(output) = cfg.output {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&output, xml.as_bytes())
            .map_err(|e| format!("write '{}' failed: {e}", output.display()))?;
        northstar_cli::ansi::ok(format!("XML metadata dumped: {}", output.display()));
    } else {
        print!("{xml}");
    }
    Ok(())
}

fn run_dump_metadata(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = discovery::read_xml_or_ytyp(&input)?;
    let logical = cfg
        .logical_path
        .unwrap_or_else(|| discovery::logical_asset_path_for_output(&cfg.root, &input));
    discovery::write_or_print_json(
        cfg.output.as_deref(),
        &metadata_projection_json(&xml, &logical)?,
    )
}

fn run_dump_dependencies(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = discovery::read_xml_or_ytyp(&input)?;
    let value = serde_json::json!({
        "schema": "northstar.ytyp.dependencies.v1",
        "input": input.to_string_lossy(),
        "dependencies": dependencies(&xml),
    });
    discovery::write_or_print_json(cfg.output.as_deref(), &value)
}

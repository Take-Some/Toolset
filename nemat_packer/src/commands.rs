use crate::diagnostics;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::{
    args::{parse_args, required_input},
    help, material, nef8,
};

const TOOL_NAME: &str = "northstar-nemat-packer";
const ACCEPTED_INPUTS: &str = "*.nemat.xml XMLtype material libraries and *.nemat NEF8 material libraries for inspect/validate/dump";
const PRODUCED_OUTPUTS: &str =
    "*.nemat.xml/XML dumps and JSON manifest/graph projections; no new runtime NEMAT assets";

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
        "create-draft" | "draft" | "new" | "pack" | "compile" | "build" => Err(
            "NEMAT authoring is retired; use northstar-ymat-packer or YDD/YDR built-in MaterialResource storage"
                .to_owned(),
        ),
        "validate" => run_validate(&raw_args[1..]),
        "doctor" | "--doctor" => {
            diagnostics::print_doctor_ok(TOOL_NAME);
            Ok(())
        }
        "inspect" => run_inspect(&raw_args[1..]),
        "dump-xml" | "dump-xmltype" => run_dump_xml(&raw_args[1..]),
        "manifest" => run_manifest(&raw_args[1..]),
        "graph" | "dependencies" => run_graph(&raw_args[1..]),
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

fn run_validate(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = read_xml_any(&input)?;
    let library = material::parse_material_xml(&xml)?;
    northstar_cli::ansi::ok(format!(
        "validated NEMAT XMLtype material library: {} entries={}",
        input.display(),
        library.materials.len()
    ));
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let bytes = fs::read(&input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    let logical = cfg
        .logical_path
        .clone()
        .unwrap_or_else(|| logical_asset_path(&cfg.root, &input));
    if is_nef8(&bytes) {
        let xml = nef8::decode_nemat_xmltype(&bytes)?;
        let value = json!({
            "container": nef8::inspect_nemat_json(&bytes, Some(&xml))?,
            "xmltype_summary": material::summary_json(&xml, &logical)?,
        });
        print_json(&value)
    } else {
        let value = material::summary_json(
            &String::from_utf8(bytes).map_err(|e| format!("input is not UTF-8 XMLtype: {e}"))?,
            &logical,
        )?;
        print_json(&value)
    }
}

fn run_dump_xml(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = read_xml_any(&input)?;
    material::parse_material_xml(&xml)?;
    if let Some(output) = cfg.output {
        write_bytes(&output, xml.as_bytes())?;
        northstar_cli::ansi::ok(format!("NEMAT XMLtype dumped: {}", output.display()));
    } else {
        print!("{xml}");
    }
    Ok(())
}

fn run_manifest(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = read_xml_any(&input)?;
    let logical = cfg
        .logical_path
        .unwrap_or_else(|| logical_asset_path(&cfg.root, &input));
    let value = material::manifest_json(&xml, &logical)?;
    if let Some(output) = cfg.output {
        write_json(&output, &value)?;
    } else {
        print_json(&value)?;
    }
    Ok(())
}

fn run_graph(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg)?;
    let xml = read_xml_any(&input)?;
    let logical = cfg
        .logical_path
        .unwrap_or_else(|| logical_asset_path(&cfg.root, &input));
    let value = material::graph_json(&xml, &logical)?;
    if let Some(output) = cfg.output {
        write_json(&output, &value)?;
    } else {
        print_json(&value)?;
    }
    Ok(())
}

fn read_xml_any(input: &Path) -> Result<String, String> {
    let bytes = fs::read(input).map_err(|e| format!("read '{}' failed: {e}", input.display()))?;
    if is_nef8(&bytes) {
        nef8::decode_nemat_xmltype(&bytes)
    } else {
        let xml = String::from_utf8(bytes)
            .map_err(|e| format!("input '{}' is not UTF-8 XMLtype: {e}", input.display()))?;
        if material::root_name(&xml).as_deref() != Some("NematMaterialLibrary") {
            return Err("authored .nemat source must be XMLtype <NematMaterialLibrary>; JSON is not a canonical material authoring format".to_owned());
        }
        Ok(xml)
    }
}

fn is_nef8(bytes: &[u8]) -> bool {
    bytes.get(0..4) == Some(b"NEF8")
}

fn write_bytes(output: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create parent '{}' failed: {e}", parent.display()))?;
        }
    }
    fs::write(output, bytes).map_err(|e| format!("write '{}' failed: {e}", output.display()))
}

fn write_json(output: &Path, value: &serde_json::Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    write_bytes(output, &bytes)?;
    northstar_cli::ansi::ok(format!("JSON projection written: {}", output.display()));
    Ok(())
}

fn print_json(value: &serde_json::Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn logical_asset_path(root: &Path, path: &Path) -> String {
    let absolute = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    let rel = absolute.strip_prefix(root).unwrap_or(&absolute);
    nef8::normalize_logical_path(&rel.to_string_lossy())
}

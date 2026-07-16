use crate::{
    args::{parse_args, required_input, required_output},
    diagnostics, discovery, help, nef8, source_doc,
};
use newengine_assets_api::{list_file_content_kind_label, LIST_FILE_CONTENT_KIND_YMAP};
use serde_json::json;
use std::{fs, path::Path};

const TOOL_NAME: &str = "northstar-ymap-packer";
const ACCEPTED_INPUTS: &str =
    "*.ymap.xml / *.ymap.json source map definitions; *.ymap compiled NEF8 map assets";
const PRODUCED_OUTPUTS: &str =
    "binary NEF8 *.ymap assets; dumped readable source bodies; JSON inspect reports";

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
        "compile" | "pack" | "build" => run_compile(&raw_args[1..]),
        "dump" | "unpack" | "dump-source" => run_dump(&raw_args[1..]),
        "inspect" | "manifest" => run_inspect(&raw_args[1..]),
        "validate" => run_validate(&raw_args[1..]),
        "accepted-inputs" | "--accepted-inputs" | "inputs" | "formats" => {
            diagnostics::print_contract(TOOL_NAME, ACCEPTED_INPUTS, PRODUCED_OUTPUTS);
            Ok(())
        }
        "doctor" | "--doctor" => run_doctor(),
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

fn run_doctor() -> Result<(), String> {
    if !nef8::ymap_content_kind_matches_descriptor() {
        return Err("YMAP content kind differs between assets-api and nef8 descriptor".to_owned());
    }
    diagnostics::print_doctor_ok(TOOL_NAME);
    northstar_cli::ansi::info(format!(
        "content_kind={} label={}",
        LIST_FILE_CONTENT_KIND_YMAP,
        list_file_content_kind_label(LIST_FILE_CONTENT_KIND_YMAP)
    ));
    Ok(())
}

fn run_compile(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    diagnostics::print_operation(
        TOOL_NAME,
        "compile",
        cfg.debug,
        ACCEPTED_INPUTS,
        PRODUCED_OUTPUTS,
    );
    if cfg.all {
        let sources = discovery::collect_sources(&cfg.root)?;
        if sources.is_empty() {
            northstar_cli::ansi::warn(format!(
                "no .ymap.xml/.ymap.json authoring sources found root='{}'",
                cfg.root.display()
            ));
            return Ok(());
        }
        for input in &sources {
            let output = discovery::output_for_source(input)?;
            let logical = discovery::infer_logical_path(&cfg.root, &output);
            compile_one(input, &output, &logical, cfg.dry_run, cfg.debug)?;
        }
        northstar_cli::ansi::ok(format!(
            "YMAP compile_all completed root='{}' compiled={} dry_run={}",
            cfg.root.display(),
            sources.len(),
            cfg.dry_run
        ));
        return Ok(());
    }
    let input = required_input(&cfg, "compile", "file.ymap.xml|file.ymap.json")?;
    let output = required_output(&cfg, "compile", "file.ymap")?;
    let logical_path = cfg
        .logical_path
        .clone()
        .unwrap_or_else(|| discovery::normalize(output.to_string_lossy().as_ref()));
    compile_one(&input, &output, &logical_path, cfg.dry_run, cfg.debug)
}

pub fn compile_one(
    input: &Path,
    output: &Path,
    logical_path: &str,
    dry_run: bool,
    debug: bool,
) -> Result<(), String> {
    let body =
        fs::read(input).map_err(|e| format!("read source failed '{}': {e}", input.display()))?;
    source_doc::validate_source_body(&body, input)?;
    let nef8 = nef8::encode_ymap_nef8(&body, logical_path)?;
    diagnostics::print_debug_value(debug, "input", input.display());
    diagnostics::print_debug_value(debug, "output", output.display());
    diagnostics::print_debug_value(debug, "logical_path", logical_path);
    diagnostics::print_debug_value(debug, "body_bytes", body.len());
    diagnostics::print_debug_value(debug, "nef8_bytes", nef8.len());
    if dry_run {
        northstar_cli::ansi::info(format!(
            "dry-run compile input='{}' output='{}' logical_path='{}' nef8_bytes={} body_bytes={}",
            input.display(),
            output.display(),
            logical_path,
            nef8.len(),
            body.len()
        ));
        return Ok(());
    }
    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|e| format!("create output directory failed '{}': {e}", parent.display()))?;
    }
    fs::write(output, &nef8)
        .map_err(|e| format!("write .ymap failed '{}': {e}", output.display()))?;
    northstar_cli::ansi::ok(format!(
        "compiled input='{}' output='{}' logical_path='{}' nef8_bytes={} body_bytes={}",
        input.display(),
        output.display(),
        logical_path,
        nef8.len(),
        body.len()
    ));
    Ok(())
}

fn run_dump(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "dump", "file.ymap")?;
    let bytes =
        fs::read(&input).map_err(|e| format!("read .ymap failed '{}': {e}", input.display()))?;
    let (_header, body) = nef8::decode_ymap_nef8(&bytes)?;
    source_doc::validate_source_body(&body, &input)?;
    if let Some(output) = cfg.output {
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create dump directory failed '{}': {e}", parent.display()))?;
        }
        fs::write(&output, &body)
            .map_err(|e| format!("write dump failed '{}': {e}", output.display()))?;
        northstar_cli::ansi::ok(format!(
            "dumped input='{}' output='{}' body_bytes={}",
            input.display(),
            output.display(),
            body.len()
        ));
    } else {
        let text = String::from_utf8(body)
            .map_err(|e| format!("decoded .ymap body is not UTF-8 '{}': {e}", input.display()))?;
        print!("{text}");
    }
    Ok(())
}

fn run_inspect(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    let input = required_input(&cfg, "inspect", "file.ymap")?;
    let bytes =
        fs::read(&input).map_err(|e| format!("read .ymap failed '{}': {e}", input.display()))?;
    let (header, body) = nef8::decode_ymap_nef8(&bytes)?;
    let report = json!({
        "schema": "northstar.ymap.inspect.v1",
        "input": discovery::normalize(input.to_string_lossy().as_ref()),
        "file_bytes": bytes.len(),
        "content_kind": header.content_kind,
        "content_kind_label": list_file_content_kind_label(u32::from(header.content_kind)),
        "version": header.version,
        "header_len": header.header_len,
        "body_offset": header.body_offset,
        "body_len": header.body_len,
        "body_uncompressed_len": header.body_uncompressed_len,
        "entry_count": header.entry_count,
        "stable_file_id": format!("{:016x}", header.stable_file_id),
        "body_hash_blake3": source_doc::hex_lower(&header.body_hash),
        "body_root": source_doc::body_root_label(&body),
        "policy": "runtime must consume .ymap via AssetManager decode_v1; authoring tools may dump/compile NEF8 envelopes"
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|e| format!("inspect report encode failed: {e}"))?
    );
    Ok(())
}

fn run_validate(args: &[String]) -> Result<(), String> {
    let cfg = parse_args(args)?;
    if cfg.all {
        let files = discovery::collect_ymaps(&cfg.root)?;
        if files.is_empty() {
            northstar_cli::ansi::warn(format!(
                "no .ymap files found root='{}'",
                cfg.root.display()
            ));
            return Ok(());
        }
        for file in &files {
            validate_file(file)?;
        }
        northstar_cli::ansi::ok(format!(
            "YMAP validate_all ok root='{}' files={}",
            cfg.root.display(),
            files.len()
        ));
        return Ok(());
    }
    let input = required_input(&cfg, "validate", "file.ymap")?;
    validate_file(&input)
}

fn validate_file(input: &Path) -> Result<(), String> {
    let bytes =
        fs::read(input).map_err(|e| format!("read .ymap failed '{}': {e}", input.display()))?;
    let (header, body) = nef8::decode_ymap_nef8(&bytes)?;
    if u32::from(header.content_kind) != LIST_FILE_CONTENT_KIND_YMAP {
        return Err(format!(
            "'{}' content_kind={} expected={} ({})",
            input.display(),
            header.content_kind,
            LIST_FILE_CONTENT_KIND_YMAP,
            list_file_content_kind_label(LIST_FILE_CONTENT_KIND_YMAP)
        ));
    }
    source_doc::validate_source_body(&body, input)?;
    northstar_cli::ansi::ok(format!(
        "validate ok input='{}' body_bytes={} root='{}'",
        input.display(),
        body.len(),
        source_doc::body_root_label(&body)
    ));
    Ok(())
}

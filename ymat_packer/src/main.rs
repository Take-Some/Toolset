use newengine_assets_api::decode_list_file_envelope;
use serde_json::json;
use std::{env, fs, path::Path};

fn main() {
    if let Err(error) = run() {
        eprintln!("northstar-ymat-packer: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_owned());
    match command.as_str() {
        "pack" => {
            let input = args.next().ok_or("pack requires INPUT.ymat.json")?;
            let output = args.next().ok_or("pack requires OUTPUT.ymat")?;
            let body = fs::read(&input).map_err(|e| format!("read '{input}': {e}"))?;
            let document = northstar_ymat_packer::validate_document(&body)?;
            let entry_count = northstar_ymat_packer::material_count(&document)?;
            let bytes = northstar_ymat_packer::pack_document(&document)?;
            if let Some(parent) = Path::new(&output).parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("create '{}': {e}", parent.display()))?;
            }
            fs::write(&output, &bytes).map_err(|e| format!("write '{output}': {e}"))?;
            let verify = decode_list_file_envelope(
                &bytes,
                northstar_ymat_packer::CONTENT_KIND_YMAT,
                &output,
            )?;
            northstar_ymat_packer::validate_document(&verify.body)?;
            println!(
                "YMAT_PACK_OK entries={entry_count} bytes={} output='{}'",
                bytes.len(),
                output
            );
            Ok(())
        }
        "validate" => {
            let input = args.next().ok_or("validate requires INPUT")?;
            let bytes = fs::read(&input).map_err(|e| format!("read '{input}': {e}"))?;
            let document = northstar_ymat_packer::decode_document(&bytes, &input)?;
            println!(
                "YMAT_VALIDATE_OK entries={} input='{}'",
                northstar_ymat_packer::material_count(&document)?,
                input
            );
            Ok(())
        }
        "inspect" => {
            let input = args.next().ok_or("inspect requires INPUT.ymat")?;
            let bytes = fs::read(&input).map_err(|e| format!("read '{input}': {e}"))?;
            let decoded = decode_list_file_envelope(
                &bytes,
                northstar_ymat_packer::CONTENT_KIND_YMAT,
                &input,
            )?;
            let document = northstar_ymat_packer::validate_document(&decoded.body)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "schema": northstar_ymat_packer::SCHEMA,
                    "content_kind": northstar_ymat_packer::CONTENT_KIND_YMAT,
                    "content_schema_version": decoded.header.content_schema_version,
                    "entry_count": decoded.header.entry_count,
                    "materials": document["materials"],
                }))
                .map_err(|e| e.to_string())?
            );
            Ok(())
        }
        "help" | "--help" | "-h" => {
            println!("northstar-ymat-packer");
            println!("  pack INPUT.ymat.json OUTPUT.ymat");
            println!("  validate INPUT.ymat.json|INPUT.ymat");
            println!("  inspect INPUT.ymat");
            println!();
            println!("YMAT is optional external storage for MaterialResource.");
            println!("Models may instead embed the same material semantic object in YDD/YDR.");
            Ok(())
        }
        other => Err(format!("unknown command '{other}'")),
    }
}

use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{decode_list_file_envelope, encode_list_file, ListFileEncodeRequest};
use serde_json::Value;
use std::io::Write;

pub const CONTENT_KIND_YMAT: u32 = 30;
pub const CONTENT_SCHEMA_VERSION: u16 = 1;
pub const SCHEMA: &str = "northstar.ymat.v1";

pub fn validate_document(bytes: &[u8]) -> Result<Value, String> {
    let document: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("YMAT JSON parse failed: {e}"))?;
    validate_value(&document)?;
    Ok(document)
}

pub fn validate_value(document: &Value) -> Result<(), String> {
    if document.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(format!(
            "YMAT schema mismatch: actual='{}' expected='{SCHEMA}'",
            document
                .get("schema")
                .and_then(Value::as_str)
                .unwrap_or("<missing>")
        ));
    }
    let materials = document
        .get("materials")
        .and_then(Value::as_array)
        .ok_or("YMAT requires array 'materials'")?;
    if materials.is_empty() {
        return Err("YMAT contains no materials".to_owned());
    }
    for (index, material) in materials.iter().enumerate() {
        let object = material
            .as_object()
            .ok_or_else(|| format!("material[{index}] must be object"))?;
        for key in ["name", "shader"] {
            if object
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
            {
                return Err(format!("material[{index}] requires non-empty '{key}'"));
            }
        }
        if let Some(textures) = object.get("textures") {
            let textures = textures
                .as_array()
                .ok_or_else(|| format!("material[{index}].textures must be array"))?;
            for (texture_index, texture) in textures.iter().enumerate() {
                for key in ["slot", "ref"] {
                    if texture
                        .get(key)
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
                    {
                        return Err(format!(
                            "material[{index}].textures[{texture_index}] requires '{key}'"
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn material_count(document: &Value) -> Result<u32, String> {
    let len = document
        .get("materials")
        .and_then(Value::as_array)
        .ok_or("YMAT requires materials")?
        .len();
    u32::try_from(len).map_err(|_| "YMAT material count exceeds u32".to_owned())
}

pub fn pack_document(document: &Value) -> Result<Vec<u8>, String> {
    validate_value(document)?;
    let body = serde_json::to_vec(document).map_err(|e| format!("YMAT JSON encode failed: {e}"))?;
    pack_body(&body, material_count(document)?)
}

pub fn decode_document(bytes: &[u8], label: &str) -> Result<Value, String> {
    let body = if bytes.starts_with(b"NEF8") {
        decode_list_file_envelope(bytes, CONTENT_KIND_YMAT, label)?.body
    } else {
        bytes.to_vec()
    };
    validate_document(&body)
}

pub fn select_material<'a>(document: &'a Value, name: &str) -> Result<&'a Value, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("YMAT material selector is empty".to_owned());
    }
    document
        .get("materials")
        .and_then(Value::as_array)
        .and_then(|materials| {
            materials.iter().find(|material| {
                material
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
            })
        })
        .ok_or_else(|| format!("YMAT material '{name}' not found"))
}

fn pack_body(body: &[u8], entry_count: u32) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(body).map_err(|e| e.to_string())?;
    let stored = encoder.finish().map_err(|e| e.to_string())?;
    encode_list_file(ListFileEncodeRequest {
        content_kind: CONTENT_KIND_YMAT,
        content_schema_version: CONTENT_SCHEMA_VERSION,
        entry_count,
        additional_flags: 0,
        min_size_class: 5,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: Some(*blake3::hash(body).as_bytes()),
        stable_file_id: None,
        import_settings_hash: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_round_trip() {
        let document = serde_json::json!({
            "schema": SCHEMA,
            "materials": [{
                "name": "m00",
                "shader": "pbr.default",
                "textures": [{"slot":"base_color","ref":"textures/a.ytd@m00_base"}]
            }]
        });
        let bytes = pack_document(&document).unwrap();
        let decoded = decode_document(&bytes, "test.ymat").unwrap();
        assert_eq!(decoded["materials"][0]["name"], "m00");
    }

    #[test]
    fn select_material_is_case_insensitive() {
        let document = serde_json::json!({
            "schema": SCHEMA,
            "materials": [{"name":"Body","shader":"pbr.default"}]
        });
        assert_eq!(select_material(&document, "body").unwrap()["name"], "Body");
    }
}

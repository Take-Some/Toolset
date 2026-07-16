use serde_json::{json, Value};

use crate::xmlmeta::{entry_names, normalize_logical_path, summary_json, validate_metadata_xml};

pub fn compile_ytyp_xml_to_properties(
    xml: &str,
    logical_path: &str,
    _seed: &str,
    _requested_entry_count: u64,
) -> Result<Vec<u8>, String> {
    let logical_path = normalize_logical_path(logical_path);
    validate_metadata_xml(xml, &logical_path)?;
    Ok(xml.as_bytes().to_vec())
}

pub fn decode_ytyp_xml(bytes: &[u8]) -> Result<String, String> {
    if bytes.get(0..4) == Some(b"NEF8") {
        return Err(
            "legacy NEF8 .ytyp is no longer accepted; .ytyp is raw Y-Type Properties XML"
                .to_owned(),
        );
    }
    let xml = String::from_utf8(bytes.to_vec())
        .map_err(|e| format!(".ytyp properties body is not UTF-8 XML: {e}"))?;
    validate_metadata_xml(&xml, ".ytyp")?;
    Ok(xml)
}

pub fn inspect_ytyp_json(bytes: &[u8]) -> Result<Value, String> {
    let xml = decode_ytyp_xml(bytes)?;
    let entries = entry_names(&xml);
    Ok(json!({
        "schema": "northstar.ytyp.inspect.v1",
        "ok": true,
        "container": "newengine.properties.xml.ytyp",
        "content_kind": "y_type_properties",
        "dictionary": false,
        "entry_count": entries.len(),
        "body_len": bytes.len(),
        "metadata": summary_json(&xml),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ytyp_rejects_legacy_nef8_magic() {
        assert!(inspect_ytyp_json(b"NEF8")
            .unwrap_err()
            .contains("legacy NEF8"));
    }

    #[test]
    fn ytyp_roundtrip_properties_xml() {
        let xml = r#"<?xml version="1.0"?><YtypProperties schema="newengine.ytyp.properties.v1" name="foo"><Dependencies><Dependency ref="assets/a.ytd@bar" /></Dependencies></YtypProperties>"#;
        let bytes = compile_ytyp_xml_to_properties(xml, "assets/meta/foo.ytyp", "test", 1).unwrap();
        assert!(decode_ytyp_xml(&bytes).unwrap().contains("YtypProperties"));
        assert_eq!(inspect_ytyp_json(&bytes).unwrap()["ok"], true);
    }
}

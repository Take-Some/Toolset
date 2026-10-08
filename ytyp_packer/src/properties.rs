use std::io::{Read, Write};

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use northstar_nef8::{
    body_slice, encode, entry_ref, list_file_content_kind_label, metadata_slice,
    AssetEntryDependency, AssetEntryManifest, AssetGatewayRoute, EncodeRequest,
    ListFileHeaderMetadata, ENGINE_ASSETS_DEFINITIONS_SERVICE_ID, FULL_HASH_BODY_THRESHOLD,
    TYPE_YTYP,
};
use serde_json::{json, Value};

use crate::xmlmeta::{
    dependency_records, entry_kind, entry_names, normalize_logical_path, summary_json,
    validate_metadata_xml,
};

const CONTENT_SCHEMA_VERSION: u16 = 1;
const YTYP_ROUTE_METHOD: &str = "assets.definitions.entry_v1";
const YTYP_SEMANTIC_OWNER: &str = "definition";

pub fn compile_ytyp_xml_to_properties(
    xml: &str,
    logical_path: &str,
    source_ref: &str,
    requested_entry_count: u64,
) -> Result<Vec<u8>, String> {
    validate_metadata_xml(xml, logical_path)?;
    let logical_path = normalize_logical_path(logical_path);
    if logical_path.is_empty() {
        return Err("YTYP logical path must not be empty".to_owned());
    }

    let names = entry_names(xml);
    let name = names
        .first()
        .cloned()
        .unwrap_or_else(|| fallback_entry_name(&logical_path));
    let actual_entry_count = 1u64;
    if requested_entry_count != 0 && requested_entry_count != actual_entry_count {
        return Err(format!(
            "YTYP entry count mismatch logical_path='{logical_path}' requested={requested_entry_count} actual={actual_entry_count}"
        ));
    }

    let dependencies = dependency_records(xml)
        .into_iter()
        .map(|record| {
            let dependency = if record.required {
                AssetEntryDependency::required(record.reference, record.role.clone())
            } else {
                AssetEntryDependency::optional(record.reference, record.role.clone())
            };
            dependency.domain(record.domain).role(record.role)
        })
        .collect::<Vec<_>>();

    let asset_kind = entry_kind(xml);
    let mut entry = AssetEntryManifest::new(&name, &asset_kind, entry_ref(&logical_path, &name));
    entry.route = AssetGatewayRoute::new(
        ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        YTYP_ROUTE_METHOD,
        YTYP_SEMANTIC_OWNER,
    );
    entry.dependencies = dependencies.clone();

    let metadata = ListFileHeaderMetadata {
        logical_path: logical_path.clone(),
        content_kind: list_file_content_kind_label(u32::from(TYPE_YTYP)).to_owned(),
        authored_by: "northstar-ytyp-packer".to_owned(),
        source: normalize_logical_path(source_ref),
        build_profile: "native".to_owned(),
        entries: vec![entry],
        dependencies,
        policy: vec![
            "YTYP runtime assets are canonical NEF8 V2 ListFiles".to_owned(),
            "YTYP XML is the domain body; raw XML under the .ytyp extension is not runtime-valid"
                .to_owned(),
            "YTYP entry semantics are owned by engine.assets.definitions".to_owned(),
            "runtime references use file.ytyp@entry".to_owned(),
        ],
        ..Default::default()
    };
    let metadata_bytes = serde_json::to_vec(&metadata)
        .map_err(|error| format!("YTYP header metadata JSON encode failed: {error}"))?;

    let raw_body = xml.as_bytes();
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(raw_body)
        .map_err(|error| format!("YTYP deflate write failed: {error}"))?;
    let stored_body = encoder
        .finish()
        .map_err(|error| format!("YTYP deflate finish failed: {error}"))?;
    let body_hash =
        (raw_body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(raw_body).as_bytes());

    let encoded = encode(EncodeRequest {
        content_kind: TYPE_YTYP,
        content_schema_version: CONTENT_SCHEMA_VERSION,
        entry_count: 1,
        additional_flags: 0,
        min_size_class: 4,
        metadata: &metadata_bytes,
        stored_body: &stored_body,
        body_uncompressed_len: raw_body.len() as u64,
        body_hash,
        stable_file_id: None,
        import_settings_hash: None,
    })?;

    // Producer self-check: output must pass the same public decoder used by
    // inspect/validate before it is written to disk.
    let roundtrip = decode_ytyp_document(&encoded)?;
    if roundtrip.xml.as_bytes() != raw_body {
        return Err("YTYP producer self-check changed XML domain body bytes".to_owned());
    }
    Ok(encoded)
}

pub fn decode_ytyp_xml(bytes: &[u8]) -> Result<String, String> {
    Ok(decode_ytyp_document(bytes)?.xml)
}

pub fn inspect_ytyp_json(bytes: &[u8]) -> Result<Value, String> {
    let decoded = decode_ytyp_document(bytes)?;
    let entries = entry_names(&decoded.xml);
    Ok(json!({
        "schema": "northstar.ytyp.inspect.v2",
        "ok": true,
        "container": "newengine.listfile.nef8.ytyp",
        "wire_version": decoded.header.version,
        "content_kind_id": decoded.header.content_kind,
        "content_kind": decoded.metadata.content_kind,
        "dictionary": true,
        "entry_count": decoded.header.entry_count,
        "body_len": decoded.xml.len(),
        "metadata": decoded.metadata,
        "summary": summary_json(&decoded.xml),
        "entries": entries,
    }))
}

struct DecodedYtyp {
    header: northstar_nef8::Header,
    metadata: ListFileHeaderMetadata,
    xml: String,
}

fn decode_ytyp_document(bytes: &[u8]) -> Result<DecodedYtyp, String> {
    if bytes.get(0..4) != Some(&northstar_nef8::MAGIC) {
        return Err(
            "raw XML .ytyp runtime assets are retired; expected canonical NEF8 V2 content_kind=YTYP(3)"
                .to_owned(),
        );
    }
    let header = northstar_nef8::parse_header(bytes)?;
    if header.content_kind != TYPE_YTYP {
        return Err(format!(
            "YTYP NEF8 content kind mismatch: got={} expected={TYPE_YTYP}",
            header.content_kind
        ));
    }
    if !header.is_deflate() {
        return Err("YTYP NEF8 body must use canonical raw DEFLATE".to_owned());
    }
    if !header.has_metadata() {
        return Err("YTYP NEF8 requires canonical header metadata".to_owned());
    }

    let metadata =
        serde_json::from_slice::<ListFileHeaderMetadata>(metadata_slice(bytes, &header)?)
            .map_err(|error| format!("YTYP NEF8 header metadata decode failed: {error}"))?;
    let expected_label = list_file_content_kind_label(u32::from(TYPE_YTYP));
    if metadata.content_kind != expected_label {
        return Err(format!(
            "YTYP metadata content kind mismatch: got='{}' expected='{expected_label}'",
            metadata.content_kind
        ));
    }
    if metadata.entries.len() != header.entry_count as usize {
        return Err(format!(
            "YTYP metadata/header entry count mismatch: metadata={} header={}",
            metadata.entries.len(),
            header.entry_count
        ));
    }
    if header.entry_count == 0 {
        return Err("YTYP runtime asset must expose one archetype entry".to_owned());
    }

    let mut decoder = DeflateDecoder::new(body_slice(bytes, &header)?);
    let mut raw = Vec::with_capacity(header.body_uncompressed_len as usize);
    decoder
        .read_to_end(&mut raw)
        .map_err(|error| format!("YTYP deflate decode failed: {error}"))?;
    if header.body_uncompressed_len != 0 && raw.len() as u64 != header.body_uncompressed_len {
        return Err(format!(
            "YTYP body length mismatch: got={} expected={}",
            raw.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_hash() && *blake3::hash(&raw).as_bytes() != header.body_hash {
        return Err("YTYP body BLAKE3 hash mismatch".to_owned());
    }
    let xml = String::from_utf8(raw)
        .map_err(|error| format!("YTYP XML domain body is not UTF-8: {error}"))?;
    validate_metadata_xml(&xml, &metadata.logical_path)?;

    Ok(DecodedYtyp {
        header,
        metadata,
        xml,
    })
}

fn fallback_entry_name(logical_path: &str) -> String {
    logical_path
        .rsplit('/')
        .next()
        .unwrap_or(logical_path)
        .trim_end_matches(".ytyp")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?><YtypProperties schema="newengine.ytyp.properties.v1" name="foo" entry_kind="archetype_definition"><Dependencies><Dependency reference="models/foo.ydd@foo" role="render/drawable" domain="engine.model" required="true" /></Dependencies></YtypProperties>"#;

    #[test]
    fn ytyp_runtime_contract_is_nef8_type_3() {
        let bytes = compile_ytyp_xml_to_properties(
            XML,
            "definitions/test/foo.ytyp",
            "Source/definitions/test/foo.ytyp.xml",
            1,
        )
        .unwrap();
        assert_eq!(&bytes[..4], b"NEF8");
        let header = northstar_nef8::parse_header(&bytes).unwrap();
        assert_eq!(header.version, northstar_nef8::VERSION);
        assert_eq!(header.content_kind, TYPE_YTYP);
        assert!(header.has_metadata());
        assert_eq!(header.entry_count, 1);
        assert_eq!(decode_ytyp_xml(&bytes).unwrap(), XML);
    }

    #[test]
    fn ytyp_metadata_uses_engine_owned_contract_dtos() {
        let bytes = compile_ytyp_xml_to_properties(
            XML,
            "definitions/test/foo.ytyp",
            "Source/definitions/test/foo.ytyp.xml",
            1,
        )
        .unwrap();
        let header = northstar_nef8::parse_header(&bytes).unwrap();
        let metadata = serde_json::from_slice::<ListFileHeaderMetadata>(
            metadata_slice(&bytes, &header).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata.content_kind, "ytyp_archetype_dictionary");
        assert_eq!(metadata.entries.len(), 1);
        assert_eq!(metadata.entries[0].stable_id, "dcb27518fed9d577");
        assert_eq!(metadata.entries[0].dependencies.len(), 1);
        assert_eq!(metadata.entries[0].dependencies[0].domain, "engine.model");
    }

    #[test]
    fn raw_xml_is_not_a_runtime_ytyp() {
        let error = decode_ytyp_xml(XML.as_bytes()).unwrap_err();
        assert!(error.contains("raw XML .ytyp runtime assets are retired"));
    }
}

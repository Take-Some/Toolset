use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use northstar_nef8::{
    body_slice, encode, parse_header as parse_wire_header, EncodeRequest, Header,
    FULL_HASH_BODY_THRESHOLD, TYPE_NEMAT,
};
use serde_json::{json, Value};
use std::io::{Read, Write};

pub const CONTENT_KIND_NEMAT: u16 = TYPE_NEMAT;
pub type Nef8Header = Header;

pub fn pack_nemat_xmltype(
    xml: &str,
    _logical_path: &str,
    entry_count: u64,
) -> Result<Vec<u8>, String> {
    let body = xml.as_bytes();
    let compressed = deflate(body)?;
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(body).as_bytes());
    encode(EncodeRequest {
        content_kind: CONTENT_KIND_NEMAT,
        content_schema_version: 1,
        entry_count: u32::try_from(entry_count)
            .map_err(|_| format!("NEMAT entry_count too large: {entry_count}"))?,
        additional_flags: 0,
        min_size_class: 4,
        metadata: &[],
        stored_body: &compressed,
        body_uncompressed_len: body.len() as u64,
        body_hash,
        stable_file_id: None,
        import_settings_hash: None,
    })
}

#[inline]
pub fn parse_header(bytes: &[u8]) -> Result<Nef8Header, String> {
    parse_wire_header(bytes)
}

pub fn decode_nemat_xmltype(bytes: &[u8]) -> Result<String, String> {
    let header = parse_header(bytes)?;
    if header.content_kind != CONTENT_KIND_NEMAT {
        return Err(format!(
            "NEF8 content_kind={} is not material_library ({})",
            header.content_kind, CONTENT_KIND_NEMAT
        ));
    }
    if !header.is_deflate() {
        return Err(format!(
            "NEF8 .nemat requires deflate body flags=0x{:04x} compression={}",
            header.flags, header.compression
        ));
    }
    let inflated = inflate(body_slice(bytes, &header)?)?;
    if header.body_uncompressed_len != 0 && inflated.len() as u64 != header.body_uncompressed_len {
        return Err(format!(
            "NEF8 inflated body size mismatch actual={} expected={}",
            inflated.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_hash() && blake3::hash(&inflated).as_bytes() != &header.body_hash {
        return Err("NEF8 body hash mismatch after inflate".to_owned());
    }
    let xml = String::from_utf8(inflated)
        .map_err(|error| format!(".nemat XMLtype body is not UTF-8: {error}"))?;
    if !xml.trim_start().contains("<NematMaterialLibrary") {
        return Err(".nemat XMLtype body does not contain <NematMaterialLibrary>".to_owned());
    }
    Ok(xml)
}

pub fn inspect_nemat_json(bytes: &[u8], xmltype: Option<&str>) -> Result<Value, String> {
    let header = parse_header(bytes)?;
    let ok = header.content_kind == CONTENT_KIND_NEMAT;
    let mut value = json!({
        "schema": "northstar.nemat.inspect.v1",
        "ok": ok,
        "header": {
            "magic": "NEF8",
            "version": header.version,
            "size_class": header.size_class,
            "header_len": header.header_len,
            "content_kind": header.content_kind,
            "content_kind_label": if ok { "material_library" } else { "non_nemat" },
            "flags": header.flags,
            "compression": header.compression,
            "entry_count": header.entry_count,
            "body_offset": header.body_offset,
            "body_len": header.body_len,
            "body_uncompressed_len": header.body_uncompressed_len,
            "stable_file_id": format!("{:016x}", header.stable_file_id),
            "schema_version": header.content_schema_version
        },
        "body": {
            "kind": "XMLtype",
            "root": "NematMaterialLibrary"
        }
    });
    if let Some(xml) = xmltype {
        value["xmltype"] = json!({
            "byte_len": xml.len(),
            "starts_with": xml.trim_start().lines().next().unwrap_or("")
        });
    }
    Ok(value)
}

pub fn normalize_logical_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

fn deflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    encoder.finish().map_err(|error| error.to_string())
}

fn inflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|error| format!("deflate decode failed: {error}"))?;
    Ok(out)
}

pub fn stable_u64(value: &str) -> u64 {
    fnv1a64(value.as_bytes())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nemat_roundtrip_xmltype() {
        let xml = r#"<?xml version="1.0"?><NematMaterialLibrary schema="newengine.nemat.xmltype.v1"><Entries><Material name="m" shader="pbr.default"><Textures><Texture slot="base_color" ref="textures/a.ytd@a" /></Textures></Material></Entries></NematMaterialLibrary>"#;
        let bytes = pack_nemat_xmltype(xml, "materials/test.nemat", 1).unwrap();
        let header = parse_header(&bytes).unwrap();
        assert_eq!(header.version, northstar_nef8::VERSION);
        assert_eq!(header.header_len, 32);
        assert_eq!(decode_nemat_xmltype(&bytes).unwrap(), xml);
        assert_eq!(inspect_nemat_json(&bytes, Some(xml)).unwrap()["ok"], true);
    }

    #[test]
    fn nemat_rejects_bad_magic() {
        assert!(parse_header(b"BAD!").unwrap_err().contains("prologue"));
    }
}

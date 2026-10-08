use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use newengine_texture_container::parse_manifest_only;
use northstar_nef8::{
    body_slice, encode, parse_header as parse_wire_header, EncodeRequest, Header,
    FULL_HASH_BODY_THRESHOLD, TYPE_YTD,
};
use std::io::{Read, Write};

const CONTENT_SCHEMA_VERSION: u16 = 1;
pub const CONTENT_KIND_YTD: u16 = TYPE_YTD;
pub type Nef8Header = Header;

pub fn pack_ytd(body: &[u8], _logical_path: &str) -> Result<Vec<u8>, String> {
    let compressed = deflate(body)?;
    let entry_count = parse_manifest_only(body)
        .map(|manifest| manifest.entries.len().min(u32::MAX as usize) as u32)
        .unwrap_or(1);
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(body).as_bytes());
    encode(EncodeRequest {
        content_kind: CONTENT_KIND_YTD,
        content_schema_version: CONTENT_SCHEMA_VERSION,
        entry_count,
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

pub fn decode_ytd_body(bytes: &[u8], header: &Nef8Header) -> Result<Vec<u8>, String> {
    if header.content_kind != CONTENT_KIND_YTD {
        return Err(format!(
            "not a .ytd NEF8 content_kind={} expected={}",
            header.content_kind, CONTENT_KIND_YTD
        ));
    }
    if !header.is_deflate() {
        return Err(format!(
            "unsupported NEF8 body flags=0x{:04x} compression={}",
            header.flags, header.compression
        ));
    }
    let body = inflate(body_slice(bytes, header)?)?;
    if header.body_uncompressed_len != 0 && body.len() as u64 != header.body_uncompressed_len {
        return Err(format!(
            "NEF8 inflated body size mismatch actual={} expected={}",
            body.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_hash() && blake3::hash(&body).as_bytes() != &header.body_hash {
        return Err("NEF8 body hash mismatch".to_owned());
    }
    if body.get(0..4) != Some(b"NETD") {
        return Err("YTD body is not NETD texture dictionary payload".to_owned());
    }
    Ok(body)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packer_emits_variable_header() {
        let mut body = b"NETD".to_vec();
        body.extend_from_slice(&[0; 64]);
        let packed = pack_ytd(&body, "textures/test.ytd").unwrap();
        let header = parse_header(&packed).unwrap();
        assert_eq!(header.version, northstar_nef8::VERSION);
        assert_eq!(header.header_len, 32);
    }
}

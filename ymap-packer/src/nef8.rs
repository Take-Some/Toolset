use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use northstar_nef8::{
    body_slice, encode, parse_header, EncodeRequest, Header, FULL_HASH_BODY_THRESHOLD, TYPE_YMAP,
};
use std::io::{Read, Write};

pub fn encode_ymap_nef8(body: &[u8], _logical_path: &str) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(body)
        .map_err(|error| format!("deflate write failed: {error}"))?;
    let compressed = encoder
        .finish()
        .map_err(|error| format!("deflate finish failed: {error}"))?;
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(body).as_bytes());
    encode(EncodeRequest {
        content_kind: TYPE_YMAP,
        content_schema_version: 1,
        entry_count: 0,
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

pub fn decode_ymap_nef8(bytes: &[u8]) -> Result<(Header, Vec<u8>), String> {
    let header = parse_header(bytes)?;
    if header.content_kind != TYPE_YMAP {
        return Err(format!(
            "NEF8 content_kind mismatch: got={} expected={}",
            header.content_kind, TYPE_YMAP
        ));
    }
    if !header.is_deflate() {
        return Err(format!(
            "YMAP NEF8 body must be DEFLATE flags=0x{:04x} compression={}",
            header.flags, header.compression
        ));
    }
    let compressed = body_slice(bytes, &header)?;
    let mut decoder = DeflateDecoder::new(compressed);
    let mut body = Vec::with_capacity(header.body_uncompressed_len.min(usize::MAX as u64) as usize);
    decoder
        .read_to_end(&mut body)
        .map_err(|error| format!("NEF8 deflate body decode failed: {error}"))?;
    if header.body_uncompressed_len != 0 && body.len() != header.body_uncompressed_len as usize {
        return Err(format!(
            "NEF8 inflated body length mismatch: got={} expected={}",
            body.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_hash() && header.body_hash != *blake3::hash(&body).as_bytes() {
        return Err("NEF8 inflated body BLAKE3 hash mismatch".to_owned());
    }
    Ok((header, body))
}

pub fn ymap_content_kind_matches_descriptor() -> bool {
    u32::from(TYPE_YMAP) == newengine_asset_format_nef8::ymap::CONTENT_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymap_round_trip() {
        let body = br#"{"schema":"northstar.ymap.test.v1"}"#;
        let packed = encode_ymap_nef8(body, "maps/test.ymap").unwrap();
        let (header, decoded) = decode_ymap_nef8(&packed).unwrap();
        assert_eq!(header.version, northstar_nef8::VERSION);
        assert_eq!(header.header_len, 32);
        assert_eq!(decoded, body);
    }
}

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use northstar_nef8::{
    body_slice, encode, parse_header as parse_wire_header, EncodeRequest, Header,
    FULL_HASH_BODY_THRESHOLD, TYPE_YSCD,
};
use std::io::{Read, Write};

pub const CONTENT_KIND_YSCD: u16 = TYPE_YSCD;
pub type Nef8Header = Header;

pub fn pack_yscd(body: &[u8], cue_count: usize) -> Result<Vec<u8>, String> {
    let compressed = deflate(body)?;
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(body).as_bytes());
    encode(EncodeRequest {
        content_kind: CONTENT_KIND_YSCD,
        content_schema_version: newengine_asset_format_nef8::YSCD_BINARY_SCHEMA_VERSION,
        entry_count: u32::try_from(cue_count).map_err(|_| "too many YSCD cues")?,
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

pub fn decode_body(bytes: &[u8]) -> Result<(Nef8Header, Vec<u8>), String> {
    let header = parse_header(bytes)?;
    if header.content_kind != CONTENT_KIND_YSCD {
        return Err(format!(
            "not a .yscd NEF8 content_kind={} expected={}",
            header.content_kind, CONTENT_KIND_YSCD
        ));
    }
    if header.content_schema_version != newengine_asset_format_nef8::YSCD_BINARY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported YSCD NEF8 content schema {}",
            header.content_schema_version
        ));
    }
    if !header.is_deflate() {
        return Err(format!(
            "unsupported YSCD NEF8 compression flags=0x{:04x} compression={}",
            header.flags, header.compression
        ));
    }
    let body = inflate(body_slice(bytes, &header)?)?;
    if header.body_uncompressed_len != 0 && body.len() as u64 != header.body_uncompressed_len {
        return Err(format!(
            "YSCD inflated body size mismatch actual={} expected={}",
            body.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_hash() && blake3::hash(&body).as_bytes() != &header.body_hash {
        return Err("YSCD NEF8 body BLAKE3 mismatch".to_owned());
    }
    Ok((header, body))
}

fn deflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

fn inflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| format!("YSCD deflate decode failed: {e}"))?;
    Ok(out)
}

use std::collections::HashSet;
use std::io::{Read, Write};

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use northstar_nef8::{
    body_slice, encode, parse_header as parse_wire_header, EncodeRequest, Header,
    FULL_HASH_BODY_THRESHOLD, TYPE_NEFTD,
};
use serde_json::json;

use crate::font::{stable_hash64, FontDictionary, FontKind};

pub const CONTENT_KIND_NEFTD_FONT_DICTIONARY: u16 = TYPE_NEFTD;
const BODY_VERSION: u32 = 1;
const BODY_HEADER_LEN: usize = 40;
const ENTRY_LEN: usize = 128;

pub type Nef8Header = Header;

#[derive(Debug, Clone)]
pub struct FontEntryInfo {
    pub name: String,
    pub selector: String,
    pub family: String,
    pub style: String,
    pub weight: u16,
    pub kind: String,
    pub payload_len: u64,
    pub hash_hex: String,
}

pub fn pack_neftd(
    dict: &FontDictionary,
    _logical_path: &str,
    compress: bool,
) -> Result<Vec<u8>, String> {
    if !compress {
        return Err(
            "NEF8 production contract requires DEFLATE; --no-compress is no longer supported"
                .to_owned(),
        );
    }
    validate_dictionary(dict)?;
    let body = encode_body(dict)?;
    let stored = deflate(&body)?;
    let body_hash =
        (body.len() >= FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(&body).as_bytes());
    encode(EncodeRequest {
        content_kind: CONTENT_KIND_NEFTD_FONT_DICTIONARY,
        content_schema_version: BODY_VERSION as u16,
        entry_count: dict.entries.len().min(u32::MAX as usize) as u32,
        additional_flags: 0,
        min_size_class: 4,
        metadata: &[],
        stored_body: &stored,
        body_uncompressed_len: body.len() as u64,
        body_hash,
        stable_file_id: None,
        import_settings_hash: None,
    })
}

pub fn parse_neftd(
    bytes: &[u8],
    file_name: &str,
) -> Result<(Nef8Header, Vec<FontEntryInfo>), String> {
    let header = parse_header(bytes)?;
    if !is_neftd_header(&header) {
        return Err(format!(
            "NEF8 content_kind={} is not neftd font_dictionary ({})",
            header.content_kind, CONTENT_KIND_NEFTD_FONT_DICTIONARY
        ));
    }
    let body = decode_body(bytes, &header)?;
    let entries = parse_body_index(&body, file_name)?;
    Ok((header, entries))
}

pub fn inspect_json(bytes: &[u8], file_name: &str) -> Result<serde_json::Value, String> {
    let (header, entries) = parse_neftd(bytes, file_name)?;
    let items = entries
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "selector": e.selector,
                "family": e.family,
                "style": e.style,
                "weight": e.weight,
                "kind": e.kind,
                "payload_len": e.payload_len,
                "hash_blake3": e.hash_hex,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema": "northstar.neftd.inspect.v1",
        "ok": true,
        "container": "NEF8 ListFile",
        "content_kind": "font_dictionary",
        "file": normalize_logical_path(file_name),
        "header": {
            "magic": "NEF8",
            "version": header.version,
            "size_class": header.size_class,
            "header_len": header.header_len,
            "content_kind": header.content_kind,
            "flags": header.flags,
            "compression": header.compression,
            "entry_count": header.entry_count,
            "body_offset": header.body_offset,
            "body_len": header.body_len,
            "body_uncompressed_len": header.body_uncompressed_len,
            "stable_file_id": format!("{:016x}", header.stable_file_id),
        },
        "font_dictionary": { "entry_count": entries.len(), "entries": items }
    }))
}

pub fn extract_entry(
    bytes: &[u8],
    file_name: &str,
    entry_name: &str,
) -> Result<(String, Vec<u8>), String> {
    let header = parse_header(bytes)?;
    if !is_neftd_header(&header) {
        return Err("not a .neftd font dictionary".to_owned());
    }
    let body = decode_body(bytes, &header)?;
    let (info, payload) = read_payload(&body, file_name, entry_name)?;
    Ok((info.name, payload))
}

fn validate_dictionary(dict: &FontDictionary) -> Result<(), String> {
    if dict.entries.is_empty() {
        return Err(".neftd font dictionary must contain at least one font entry".to_owned());
    }
    let mut names = HashSet::new();
    let mut hashes = HashSet::new();
    for entry in &dict.entries {
        if !names.insert(entry.name.to_ascii_lowercase()) {
            return Err(format!("duplicate font entry name '{}'", entry.name));
        }
        if !hashes.insert(stable_hash64(&entry.name)) {
            return Err(format!("duplicate font entry hash '{}'", entry.name));
        }
        if entry.bytes.is_empty() {
            return Err(format!("font entry '{}' has empty payload", entry.name));
        }
        if FontKind::from_bytes(&entry.bytes) != Some(entry.kind) {
            return Err(format!(
                "font entry '{}' signature changed before pack",
                entry.name
            ));
        }
    }
    Ok(())
}

fn encode_body(dict: &FontDictionary) -> Result<Vec<u8>, String> {
    let entry_count = dict.entries.len();
    let entry_table_offset = BODY_HEADER_LEN;
    let string_table_offset = entry_table_offset + entry_count * ENTRY_LEN;
    let mut strings = Vec::<u8>::new();
    let mut payloads = Vec::<&[u8]>::new();
    let mut records = Vec::<BuildRecord>::new();
    for entry in &dict.entries {
        let name_offset = push_string(&mut strings, &entry.name);
        let family_offset = push_string(&mut strings, &entry.family);
        let style_offset = push_string(&mut strings, &entry.style);
        let source_offset = push_string(&mut strings, &entry.source_path);
        records.push(BuildRecord {
            name: entry.name.clone(),
            name_offset,
            family_offset,
            style_offset,
            source_offset,
            kind: entry.kind.label().to_owned(),
            weight: entry.weight,
            payload_len: entry.bytes.len() as u64,
            hash: entry.hash,
        });
        payloads.push(&entry.bytes);
    }
    let payload_offset = string_table_offset + strings.len();
    let mut out =
        Vec::with_capacity(payload_offset + payloads.iter().map(|p| p.len()).sum::<usize>());
    write_u32_vec(&mut out, BODY_VERSION);
    write_u32_vec(&mut out, entry_count as u32);
    write_u64_vec(&mut out, entry_table_offset as u64);
    write_u64_vec(&mut out, string_table_offset as u64);
    write_u64_vec(&mut out, strings.len() as u64);
    write_u64_vec(&mut out, payload_offset as u64);
    let mut running = payload_offset as u64;
    for record in &records {
        write_u64_vec(&mut out, stable_hash64(&record.name));
        write_u32_vec(&mut out, record.name_offset);
        write_u32_vec(&mut out, record.family_offset);
        write_u32_vec(&mut out, record.style_offset);
        write_u32_vec(&mut out, record.source_offset);
        write_u16_vec(&mut out, record.weight);
        write_u16_vec(&mut out, kind_code(&record.kind));
        write_u32_vec(&mut out, 0);
        write_u64_vec(&mut out, running);
        write_u64_vec(&mut out, record.payload_len);
        out.extend_from_slice(&record.hash);
        write_u64_vec(&mut out, 0);
        write_u64_vec(&mut out, 0);
        write_u64_vec(&mut out, 0);
        write_u64_vec(&mut out, 0);
        write_u64_vec(&mut out, 0);
        write_u64_vec(&mut out, 0);
        running += record.payload_len;
    }
    out.extend_from_slice(&strings);
    for payload in payloads {
        out.extend_from_slice(payload);
    }
    Ok(out)
}

fn parse_body_index(body: &[u8], file_name: &str) -> Result<Vec<FontEntryInfo>, String> {
    if body.len() < BODY_HEADER_LEN {
        return Err(format!("NEFTD body too small: {}", body.len()));
    }
    let version = read_u32(body, 0)?;
    if version != BODY_VERSION {
        return Err(format!("unsupported NEFTD body version {version}"));
    }
    let count = read_u32(body, 4)? as usize;
    let table = read_u64(body, 8)? as usize;
    let strings_at = read_u64(body, 16)? as usize;
    let strings_len = read_u64(body, 24)? as usize;
    if table
        .checked_add(count * ENTRY_LEN)
        .ok_or("NEFTD table overflow")?
        > body.len()
    {
        return Err("NEFTD entry table outside body".to_owned());
    }
    if strings_at
        .checked_add(strings_len)
        .ok_or("NEFTD string table overflow")?
        > body.len()
    {
        return Err("NEFTD string table outside body".to_owned());
    }
    let strings = &body[strings_at..strings_at + strings_len];
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = table + i * ENTRY_LEN;
        let name = read_string(strings, read_u32(body, o + 8)?)?;
        let family = read_string(strings, read_u32(body, o + 12)?)?;
        let style = read_string(strings, read_u32(body, o + 16)?)?;
        let weight = read_u16(body, o + 24)?;
        let kind = kind_label(read_u16(body, o + 26)?).to_owned();
        let payload_start = read_u64(body, o + 32)? as usize;
        let payload_len = read_u64(body, o + 40)? as usize;
        if payload_start
            .checked_add(payload_len)
            .ok_or("NEFTD payload range overflow")?
            > body.len()
        {
            return Err(format!("font entry '{}' payload outside body", name));
        }
        let hash = read_hash32(body, o + 48)?;
        out.push(FontEntryInfo {
            selector: format!("{}@{}", normalize_logical_path(file_name), name),
            name,
            family,
            style,
            weight,
            kind,
            payload_len: payload_len as u64,
            hash_hex: hex32(&hash),
        });
    }
    Ok(out)
}

fn read_payload(
    body: &[u8],
    file_name: &str,
    entry_name: &str,
) -> Result<(FontEntryInfo, Vec<u8>), String> {
    let entries = parse_body_index(body, file_name)?;
    let index = entries
        .iter()
        .position(|e| e.name.eq_ignore_ascii_case(entry_name))
        .ok_or_else(|| format!("font entry '{entry_name}' not found"))?;
    let table = read_u64(body, 8)? as usize;
    let o = table + index * ENTRY_LEN;
    let start = read_u64(body, o + 32)? as usize;
    let len = read_u64(body, o + 40)? as usize;
    Ok((entries[index].clone(), body[start..start + len].to_vec()))
}

pub fn decode_body(bytes: &[u8], header: &Nef8Header) -> Result<Vec<u8>, String> {
    if !header.is_deflate() {
        return Err(format!(
            "unsupported NEF8 compression flags=0x{:04x} compression={}",
            header.flags, header.compression
        ));
    }
    let inflated = inflate(body_slice(bytes, header)?)?;
    if header.body_uncompressed_len != 0 && inflated.len() as u64 != header.body_uncompressed_len {
        return Err(format!(
            "NEF8 body size mismatch actual={} expected={}",
            inflated.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_hash() && blake3::hash(&inflated).as_bytes() != &header.body_hash {
        return Err("NEF8 body hash mismatch".to_owned());
    }
    Ok(inflated)
}

#[inline]
pub fn parse_header(bytes: &[u8]) -> Result<Nef8Header, String> {
    parse_wire_header(bytes)
}

#[inline]
fn is_neftd_header(header: &Nef8Header) -> bool {
    header.content_kind == CONTENT_KIND_NEFTD_FONT_DICTIONARY
}

struct BuildRecord {
    name: String,
    name_offset: u32,
    family_offset: u32,
    style_offset: u32,
    source_offset: u32,
    kind: String,
    weight: u16,
    payload_len: u64,
    hash: [u8; 32],
}
fn kind_code(value: &str) -> u16 {
    match value {
        "ttf" => 1,
        "otf" => 2,
        "woff" => 3,
        "woff2" => 4,
        "ttc" => 5,
        _ => 0,
    }
}
fn kind_label(value: u16) -> &'static str {
    match value {
        1 => "ttf",
        2 => "otf",
        3 => "woff",
        4 => "woff2",
        5 => "ttc",
        _ => "unknown",
    }
}
fn normalize_logical_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}
fn deflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
    e.write_all(bytes).map_err(|e| e.to_string())?;
    e.finish().map_err(|e| e.to_string())
}
fn inflate(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut d = DeflateDecoder::new(bytes);
    let mut out = Vec::new();
    d.read_to_end(&mut out)
        .map_err(|e| format!("deflate decode failed: {e}"))?;
    Ok(out)
}
fn push_string(strings: &mut Vec<u8>, value: &str) -> u32 {
    let o = strings.len() as u32;
    strings.extend_from_slice(value.as_bytes());
    strings.push(0);
    o
}
fn read_string(strings: &[u8], offset: u32) -> Result<String, String> {
    let start = offset as usize;
    if start >= strings.len() {
        return Err(format!("NEFTD string offset {offset} outside table"));
    }
    let len = strings[start..]
        .iter()
        .position(|b| *b == 0)
        .ok_or("NEFTD string is not nul-terminated")?;
    String::from_utf8(strings[start..start + len].to_vec())
        .map_err(|e| format!("NEFTD string is not UTF-8: {e}"))
}
fn hex32(v: &[u8; 32]) -> String {
    v.iter().map(|b| format!("{b:02x}")).collect()
}
fn write_u16_vec(b: &mut Vec<u8>, v: u16) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn write_u32_vec(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn write_u64_vec(b: &mut Vec<u8>, v: u64) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn read_u16(b: &[u8], o: usize) -> Result<u16, String> {
    let s = b.get(o..o + 2).ok_or("truncated u16")?;
    Ok(u16::from_le_bytes(s.try_into().unwrap()))
}
fn read_u32(b: &[u8], o: usize) -> Result<u32, String> {
    let s = b.get(o..o + 4).ok_or("truncated u32")?;
    Ok(u32::from_le_bytes(s.try_into().unwrap()))
}
fn read_u64(b: &[u8], o: usize) -> Result<u64, String> {
    let s = b.get(o..o + 8).ok_or("truncated u64")?;
    Ok(u64::from_le_bytes(s.try_into().unwrap()))
}
fn read_hash32(b: &[u8], o: usize) -> Result<[u8; 32], String> {
    let s = b.get(o..o + 32).ok_or("truncated hash32")?;
    let mut out = [0u8; 32];
    out.copy_from_slice(s);
    Ok(out)
}

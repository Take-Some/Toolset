use crate::manifest::{
    effective_clip_name, normalize_relative_source, resolve_source, ClipManifest, CueManifest,
    DictionaryManifest, OwnedCueDescriptor, MANIFEST_SCHEMA,
};
use std::collections::BTreeMap;
use std::path::Path;

pub const BODY_MAGIC: [u8; 4] = *b"YSCD";
pub const BODY_SCHEMA_VERSION: u16 = 1;
const HEADER_LEN: usize = 64;
const CUE_RECORD_LEN: usize = 40;
const CLIP_RECORD_LEN: usize = 88;
const ALIGNMENT: usize = 16;

#[derive(Debug, Clone)]
pub struct DecodedDictionary {
    pub manifest: DictionaryManifest,
    pub payloads: Vec<DecodedPayload>,
}

#[derive(Debug, Clone)]
pub struct DecodedPayload {
    pub cue: String,
    pub clip: String,
    pub source: String,
    pub codec: String,
    pub bytes: Vec<u8>,
    pub hash: [u8; 32],
}

#[derive(Default)]
struct StringPool {
    bytes: Vec<u8>,
    offsets: BTreeMap<String, u32>,
}

impl StringPool {
    fn intern(&mut self, value: &str) -> Result<u32, String> {
        if let Some(offset) = self.offsets.get(value) {
            return Ok(*offset);
        }
        let offset =
            u32::try_from(self.bytes.len()).map_err(|_| "YSCD string table exceeds u32")?;
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(0);
        self.offsets.insert(value.to_owned(), offset);
        Ok(offset)
    }
}

struct PendingCue {
    hash: u64,
    name_off: u32,
    descriptor_off: u32,
    descriptor_len: u32,
    first_clip: u32,
    clip_count: u32,
    flags: u32,
}

struct PendingClip {
    hash: u64,
    name_off: u32,
    source_off: u32,
    codec_off: u32,
    weight: f32,
    gain: f32,
    pitch: f32,
    bytes: Vec<u8>,
    hash_bytes: [u8; 32],
}

pub fn build_body(manifest: &DictionaryManifest, source_root: &Path) -> Result<Vec<u8>, String> {
    crate::manifest::validate_manifest(manifest)?;
    let mut strings = StringPool::default();
    let mut cues = Vec::with_capacity(manifest.cues.len());
    let mut clips = Vec::new();

    for cue in &manifest.cues {
        let name = cue.name.trim();
        let descriptor = cue.descriptor_json()?;
        let first_clip = u32::try_from(clips.len()).map_err(|_| "too many YSCD clips")?;
        for (index, clip) in cue.clips.iter().enumerate() {
            let clip_name = effective_clip_name(clip, index);
            let source = normalize_relative_source(&clip.source)?;
            let path = resolve_source(source_root, &source)?;
            let bytes = std::fs::read(&path).map_err(|e| {
                format!(
                    "read cue '{}' clip '{}' source '{}' failed: {e}",
                    name,
                    clip_name,
                    path.display()
                )
            })?;
            let codec = Path::new(&source)
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("bin")
                .to_ascii_lowercase();
            let hash_bytes = *blake3::hash(&bytes).as_bytes();
            clips.push(PendingClip {
                hash: stable_hash(&format!("{name}/{clip_name}")),
                name_off: strings.intern(&clip_name)?,
                source_off: strings.intern(&source)?,
                codec_off: strings.intern(&codec)?,
                weight: clip.weight,
                gain: clip.gain,
                pitch: clip.pitch,
                bytes,
                hash_bytes,
            });
        }
        cues.push(PendingCue {
            hash: stable_hash(name),
            name_off: strings.intern(name)?,
            descriptor_off: strings.intern(&descriptor)?,
            descriptor_len: u32::try_from(descriptor.len())
                .map_err(|_| "cue descriptor too large")?,
            first_clip,
            clip_count: u32::try_from(cue.clips.len()).map_err(|_| "cue clip count too large")?,
            flags: u32::from(cue.looping),
        });
    }

    let cue_table_offset = HEADER_LEN;
    let clip_table_offset = cue_table_offset
        .checked_add(
            cues.len()
                .checked_mul(CUE_RECORD_LEN)
                .ok_or("YSCD cue table overflow")?,
        )
        .ok_or("YSCD cue table overflow")?;
    let string_table_offset = clip_table_offset
        .checked_add(
            clips
                .len()
                .checked_mul(CLIP_RECORD_LEN)
                .ok_or("YSCD clip table overflow")?,
        )
        .ok_or("YSCD clip table overflow")?;
    let string_table_len = strings.bytes.len();
    let payload_offset = align_up(
        string_table_offset
            .checked_add(string_table_len)
            .ok_or("YSCD string table overflow")?,
        ALIGNMENT,
    );
    let payload_len = clips.iter().try_fold(0usize, |acc, clip| {
        let aligned = align_up(acc, ALIGNMENT);
        aligned
            .checked_add(clip.bytes.len())
            .ok_or("YSCD payload overflow")
    })?;
    let total_len = payload_offset
        .checked_add(payload_len)
        .ok_or("YSCD body overflow")?;
    let mut out = vec![0u8; total_len];

    out[0..4].copy_from_slice(&BODY_MAGIC);
    write_u16(&mut out, 4, BODY_SCHEMA_VERSION)?;
    write_u16(&mut out, 6, 0)?;
    write_u32(
        &mut out,
        8,
        u32::try_from(cues.len()).map_err(|_| "too many cues")?,
    )?;
    write_u32(
        &mut out,
        12,
        u32::try_from(clips.len()).map_err(|_| "too many clips")?,
    )?;
    write_u64(&mut out, 16, cue_table_offset as u64)?;
    write_u64(&mut out, 24, clip_table_offset as u64)?;
    write_u64(&mut out, 32, string_table_offset as u64)?;
    write_u64(&mut out, 40, string_table_len as u64)?;
    write_u64(&mut out, 48, payload_offset as u64)?;
    write_u64(&mut out, 56, payload_len as u64)?;

    for (index, cue) in cues.iter().enumerate() {
        let at = cue_table_offset + index * CUE_RECORD_LEN;
        write_u64(&mut out, at, cue.hash)?;
        write_u32(&mut out, at + 8, cue.name_off)?;
        write_u32(&mut out, at + 12, cue.descriptor_off)?;
        write_u32(&mut out, at + 16, cue.descriptor_len)?;
        write_u32(&mut out, at + 20, cue.first_clip)?;
        write_u32(&mut out, at + 24, cue.clip_count)?;
        write_u32(&mut out, at + 28, cue.flags)?;
        write_u64(&mut out, at + 32, 0)?;
    }

    out[string_table_offset..string_table_offset + string_table_len]
        .copy_from_slice(&strings.bytes);

    let mut payload_cursor = 0usize;
    for (index, clip) in clips.iter().enumerate() {
        payload_cursor = align_up(payload_cursor, ALIGNMENT);
        let absolute_payload = payload_offset + payload_cursor;
        let at = clip_table_offset + index * CLIP_RECORD_LEN;
        write_u64(&mut out, at, clip.hash)?;
        write_u32(&mut out, at + 8, clip.name_off)?;
        write_u32(&mut out, at + 12, clip.source_off)?;
        write_u32(&mut out, at + 16, clip.codec_off)?;
        write_u32(&mut out, at + 20, 0)?;
        write_f32(&mut out, at + 24, clip.weight)?;
        write_f32(&mut out, at + 28, clip.gain)?;
        write_f32(&mut out, at + 32, clip.pitch)?;
        write_u32(&mut out, at + 36, 0)?;
        write_u64(&mut out, at + 40, absolute_payload as u64)?;
        write_u64(&mut out, at + 48, clip.bytes.len() as u64)?;
        out[at + 56..at + 88].copy_from_slice(&clip.hash_bytes);
        out[absolute_payload..absolute_payload + clip.bytes.len()].copy_from_slice(&clip.bytes);
        payload_cursor += clip.bytes.len();
    }
    Ok(out)
}

pub fn parse_body(body: &[u8]) -> Result<DecodedDictionary, String> {
    if body.len() < HEADER_LEN || body.get(0..4) != Some(&BODY_MAGIC) {
        return Err("YSCD body magic mismatch or truncated header".to_owned());
    }
    let schema = read_u16(body, 4)?;
    if schema != BODY_SCHEMA_VERSION {
        return Err(format!("unsupported YSCD body schema {schema}"));
    }
    let cue_count = read_u32(body, 8)? as usize;
    let clip_count = read_u32(body, 12)? as usize;
    let cue_table_offset = to_usize(read_u64(body, 16)?, "cue table offset")?;
    let clip_table_offset = to_usize(read_u64(body, 24)?, "clip table offset")?;
    let string_table_offset = to_usize(read_u64(body, 32)?, "string table offset")?;
    let string_table_len = to_usize(read_u64(body, 40)?, "string table len")?;
    let payload_offset = to_usize(read_u64(body, 48)?, "payload offset")?;
    let payload_len = to_usize(read_u64(body, 56)?, "payload len")?;
    checked_slice(
        body,
        cue_table_offset,
        cue_count
            .checked_mul(CUE_RECORD_LEN)
            .ok_or("cue table overflow")?,
        "cue table",
    )?;
    checked_slice(
        body,
        clip_table_offset,
        clip_count
            .checked_mul(CLIP_RECORD_LEN)
            .ok_or("clip table overflow")?,
        "clip table",
    )?;
    let strings = checked_slice(body, string_table_offset, string_table_len, "string table")?;
    checked_slice(body, payload_offset, payload_len, "payload region")?;

    #[derive(Clone)]
    struct ClipRow {
        name: String,
        source: String,
        codec: String,
        weight: f32,
        gain: f32,
        pitch: f32,
        bytes: Vec<u8>,
        hash: [u8; 32],
    }

    let mut clip_rows = Vec::with_capacity(clip_count);
    for index in 0..clip_count {
        let at = clip_table_offset + index * CLIP_RECORD_LEN;
        let name = read_cstr(strings, read_u32(body, at + 8)? as usize)?;
        let source = read_cstr(strings, read_u32(body, at + 12)? as usize)?;
        let codec = read_cstr(strings, read_u32(body, at + 16)? as usize)?;
        let payload_at = to_usize(read_u64(body, at + 40)?, "clip payload offset")?;
        let payload_len = to_usize(read_u64(body, at + 48)?, "clip payload len")?;
        let bytes = checked_slice(body, payload_at, payload_len, "clip payload")?.to_vec();
        let mut expected_hash = [0u8; 32];
        expected_hash.copy_from_slice(checked_slice(body, at + 56, 32, "clip hash")?);
        let actual_hash = *blake3::hash(&bytes).as_bytes();
        if actual_hash != expected_hash {
            return Err(format!("YSCD clip '{}' BLAKE3 mismatch", name));
        }
        clip_rows.push(ClipRow {
            name,
            source,
            codec,
            weight: read_f32(body, at + 24)?,
            gain: read_f32(body, at + 28)?,
            pitch: read_f32(body, at + 32)?,
            bytes,
            hash: expected_hash,
        });
    }

    let mut cues = Vec::with_capacity(cue_count);
    let mut payloads = Vec::with_capacity(clip_count);
    for index in 0..cue_count {
        let at = cue_table_offset + index * CUE_RECORD_LEN;
        let name = read_cstr(strings, read_u32(body, at + 8)? as usize)?;
        let descriptor = read_cstr(strings, read_u32(body, at + 12)? as usize)?;
        let descriptor_len = read_u32(body, at + 16)? as usize;
        if descriptor.as_bytes().len() != descriptor_len {
            return Err(format!("cue '{}' descriptor length mismatch", name));
        }
        let meta: OwnedCueDescriptor = serde_json::from_str(&descriptor)
            .map_err(|e| format!("cue '{}' descriptor JSON invalid: {e}", name))?;
        let first = read_u32(body, at + 20)? as usize;
        let count = read_u32(body, at + 24)? as usize;
        let end = first.checked_add(count).ok_or("cue clip range overflow")?;
        if end > clip_rows.len() {
            return Err(format!("cue '{}' clip range out of bounds", name));
        }
        let mut cue_clips = Vec::with_capacity(count);
        for row in &clip_rows[first..end] {
            cue_clips.push(ClipManifest {
                name: row.name.clone(),
                source: row.source.clone(),
                weight: row.weight,
                gain: row.gain,
                pitch: row.pitch,
            });
            payloads.push(DecodedPayload {
                cue: name.clone(),
                clip: row.name.clone(),
                source: row.source.clone(),
                codec: row.codec.clone(),
                bytes: row.bytes.clone(),
                hash: row.hash,
            });
        }
        cues.push(CueManifest {
            name,
            route: meta.route,
            looping: meta.looping,
            concurrency_group: meta.concurrency_group,
            concurrency_limit: meta.concurrency_limit,
            concurrency_scope: meta.concurrency_scope,
            steal_rule: meta.steal_rule,
            voice_budget: meta.voice_budget,
            priority: meta.priority,
            repeat_avoidance: meta.repeat_avoidance,
            spatial_policy: meta.spatial_policy,
            gain_range: meta.gain_range,
            pitch_range: meta.pitch_range,
            attenuation: meta.attenuation,
            layers: meta.layers,
            sound_graph: meta.sound_graph,
            clips: cue_clips,
        });
    }
    let manifest = DictionaryManifest {
        schema: MANIFEST_SCHEMA.to_owned(),
        version: 1,
        cues,
    };
    crate::manifest::validate_manifest(&manifest)?;
    Ok(DecodedDictionary { manifest, payloads })
}

pub fn inspect_json(decoded: &DecodedDictionary) -> serde_json::Value {
    let cues = decoded
        .manifest
        .cues
        .iter()
        .map(|cue| {
            let clips = cue
                .clips
                .iter()
                .filter_map(|clip| {
                    decoded
                        .payloads
                        .iter()
                        .find(|p| p.cue == cue.name && p.clip == clip.name)
                        .map(|p| {
                            serde_json::json!({
                                "name": clip.name,
                                "source": clip.source,
                                "codec": p.codec,
                                "bytes": p.bytes.len(),
                                "blake3": hex(&p.hash),
                                "weight": clip.weight,
                                "gain": clip.gain,
                                "pitch": clip.pitch,
                            })
                        })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "name": cue.name,
                "route": cue.route,
                "looping": cue.looping,
                "concurrency_group": cue.concurrency_group,
                "concurrency_limit": cue.concurrency_limit,
                "concurrency_scope": cue.concurrency_scope,
                "steal_rule": cue.steal_rule,
                "voice_budget": cue.voice_budget,
                "priority": cue.priority,
                "repeat_avoidance": cue.repeat_avoidance,
                "spatial_policy": cue.spatial_policy,
                "gain_range": cue.gain_range,
                "pitch_range": cue.pitch_range,
                "attenuation": cue.attenuation,
                "layers": cue.layers,
                "sound_graph": cue.sound_graph,
                "clips": clips,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema": "northstar.yscd.inspect.v1",
        "body_magic": "YSCD",
        "body_schema": BODY_SCHEMA_VERSION,
        "cue_count": decoded.manifest.cues.len(),
        "clip_count": decoded.payloads.len(),
        "embedded_audio_bytes": decoded.payloads.iter().map(|p| p.bytes.len()).sum::<usize>(),
        "cues": cues,
    })
}

pub fn write_unpacked(
    decoded: &DecodedDictionary,
    out_dir: &Path,
    overwrite: bool,
) -> Result<(), String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("create '{}' failed: {e}", out_dir.display()))?;
    let manifest_path = out_dir.join("manifest.json");
    write_file(
        &manifest_path,
        &serde_json::to_vec_pretty(&decoded.manifest).map_err(|e| e.to_string())?,
        overwrite,
    )?;
    let mut written: BTreeMap<String, [u8; 32]> = BTreeMap::new();
    for payload in &decoded.payloads {
        let source = normalize_relative_source(&payload.source)?;
        if let Some(existing) = written.get(&source) {
            if existing != &payload.hash {
                return Err(format!(
                    "unpack source collision '{}' has different payload hashes",
                    source
                ));
            }
            continue;
        }
        let path = out_dir.join(source.replace('/', std::path::MAIN_SEPARATOR_STR));
        write_file(&path, &payload.bytes, overwrite)?;
        written.insert(source, payload.hash);
    }
    Ok(())
}

fn write_file(path: &Path, bytes: &[u8], overwrite: bool) -> Result<(), String> {
    if path.exists() && !overwrite {
        return Err(format!(
            "output '{}' exists; use --overwrite",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create '{}' failed: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("write '{}' failed: {e}", path.display()))
}

fn stable_hash(text: &str) -> u64 {
    let hash = blake3::hash(text.as_bytes());
    u64::from_le_bytes(
        hash.as_bytes()[0..8]
            .try_into()
            .expect("8-byte hash prefix"),
    )
}

fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("{label} range overflow"))?;
    bytes.get(offset..end).ok_or_else(|| {
        format!(
            "{label} truncated offset={offset} len={len} bytes={}",
            bytes.len()
        )
    })
}

fn read_cstr(strings: &[u8], offset: usize) -> Result<String, String> {
    let tail = strings
        .get(offset..)
        .ok_or("YSCD string offset out of bounds")?;
    let len = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or("YSCD string is not NUL terminated")?;
    String::from_utf8(tail[..len].to_vec()).map_err(|e| format!("YSCD string is not UTF-8: {e}"))
}

fn to_usize(value: u64, label: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("{label} does not fit usize: {value}"))
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        checked_slice(bytes, at, 2, "u16")?.try_into().unwrap(),
    ))
}
fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        checked_slice(bytes, at, 4, "u32")?.try_into().unwrap(),
    ))
}
fn read_u64(bytes: &[u8], at: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        checked_slice(bytes, at, 8, "u64")?.try_into().unwrap(),
    ))
}
fn read_f32(bytes: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_le_bytes(
        checked_slice(bytes, at, 4, "f32")?.try_into().unwrap(),
    ))
}
fn write_u16(bytes: &mut [u8], at: usize, value: u16) -> Result<(), String> {
    checked_slice_mut(bytes, at, 2, "u16")?.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn write_u32(bytes: &mut [u8], at: usize, value: u32) -> Result<(), String> {
    checked_slice_mut(bytes, at, 4, "u32")?.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn write_u64(bytes: &mut [u8], at: usize, value: u64) -> Result<(), String> {
    checked_slice_mut(bytes, at, 8, "u64")?.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn write_f32(bytes: &mut [u8], at: usize, value: f32) -> Result<(), String> {
    checked_slice_mut(bytes, at, 4, "f32")?.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn checked_slice_mut<'a>(
    bytes: &'a mut [u8],
    offset: usize,
    len: usize,
    label: &str,
) -> Result<&'a mut [u8], String> {
    let total = bytes.len();
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("{label} range overflow"))?;
    bytes.get_mut(offset..end).ok_or_else(|| {
        format!("{label} write out of bounds offset={offset} len={len} bytes={total}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ClipManifest, CueManifest};

    #[test]
    fn body_round_trip_preserves_embedded_bytes() {
        let root = std::env::temp_dir().join(format!("yscd-body-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("audio")).unwrap();
        let source_bytes = b"RIFFsynthetic-wave-payload";
        std::fs::write(root.join("audio/fire.wav"), source_bytes).unwrap();
        let manifest = DictionaryManifest {
            cues: vec![CueManifest {
                name: "fire".to_owned(),
                clips: vec![ClipManifest {
                    name: "fire_a".to_owned(),
                    source: "audio/fire.wav".to_owned(),
                    ..ClipManifest::default()
                }],
                ..CueManifest::default()
            }],
            ..DictionaryManifest::default()
        };
        let body = build_body(&manifest, &root).unwrap();
        let decoded = parse_body(&body).unwrap();
        assert_eq!(decoded.manifest.cues[0].name, "fire");
        assert_eq!(decoded.payloads[0].bytes, source_bytes);
        let _ = std::fs::remove_dir_all(&root);
    }
}

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use flate2::{read::DeflateDecoder, write::DeflateEncoder, Compression};
use serde_json::json;

use crate::fswalk::{safe_output_path, validate_package_path, SourceFile};

const MAGIC: &[u8; 8] = b"NEPAK\0\0\0";
const VERSION_MAJOR: u16 = 1;
const VERSION_MINOR: u16 = 0;
const HEADER_LEN: usize = 128;
const ENTRY_LEN: usize = 128;
const ENDIAN_LITTLE: u8 = 1;
const SECTOR_SHIFT: u8 = 9;
const SECTOR_SIZE: u64 = 1 << SECTOR_SHIFT;
const ROOT_PARENT: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind { Directory = 1, File = 2, Resource = 3 }

impl EntryKind {
    fn from_u16(value: u16) -> Result<Self, String> {
        match value { 1 => Ok(Self::Directory), 2 => Ok(Self::File), 3 => Ok(Self::Resource), _ => Err(format!("unsupported NEPAK entry kind {value}")) }
    }
    fn as_str(self) -> &'static str { match self { Self::Directory => "directory", Self::File => "file", Self::Resource => "resource" } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionKind { None = 0, Deflate = 1 }

impl CompressionKind {
    fn from_u16(value: u16) -> Result<Self, String> {
        match value { 0 => Ok(Self::None), 1 => Ok(Self::Deflate), _ => Err(format!("unsupported NEPAK compression kind {value}")) }
    }
    fn as_str(self) -> &'static str { match self { Self::None => "none", Self::Deflate => "deflate" } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    OpaqueFile = 0,
    VfsPackage = 1,
    TextureDictionary = 2,
    DrawableDictionary = 3,
    ArchetypeDictionary = 4,
    MaterialLibrary = 5,
    AiPatternDictionary = 6,
    UiDocument = 7,
}

impl ContentKind {
    fn from_u16(value: u16) -> Result<Self, String> {
        match value {
            0 => Ok(Self::OpaqueFile), 1 => Ok(Self::VfsPackage), 2 => Ok(Self::TextureDictionary),
            3 => Ok(Self::DrawableDictionary), 4 => Ok(Self::ArchetypeDictionary), 5 => Ok(Self::MaterialLibrary),
            6 => Ok(Self::AiPatternDictionary), 7 => Ok(Self::UiDocument), _ => Err(format!("unsupported NEPAK content kind {value}")),
        }
    }
    fn for_path(path: &str) -> Self {
        match lower_extension(path).as_deref() {
            Some("nepak") => Self::VfsPackage,
            Some("ytd") => Self::TextureDictionary,
            Some("ydd") => Self::DrawableDictionary,
            Some("ytyp") => Self::ArchetypeDictionary,
            Some("nemat") => Self::MaterialLibrary,
            Some("nepat") => Self::AiPatternDictionary,
            Some("neui") => Self::UiDocument,
            _ => Self::OpaqueFile,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::OpaqueFile => "opaque_file", Self::VfsPackage => "vfs_package", Self::TextureDictionary => "texture_dictionary",
            Self::DrawableDictionary => "drawable_dictionary", Self::ArchetypeDictionary => "archetype_dictionary",
            Self::MaterialLibrary => "material_library", Self::AiPatternDictionary => "ai_pattern_dictionary", Self::UiDocument => "ui_document",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass { RawFile = 0, Directory = 1, ListFile = 2 }

impl StorageClass {
    fn from_u16(value: u16) -> Result<Self, String> {
        match value { 0 => Ok(Self::RawFile), 1 => Ok(Self::Directory), 2 => Ok(Self::ListFile), _ => Err(format!("unsupported NEPAK storage class {value}")) }
    }
    fn for_content(kind: ContentKind, is_dir: bool) -> Self {
        if is_dir { return Self::Directory; }
        match kind {
            ContentKind::TextureDictionary | ContentKind::DrawableDictionary | ContentKind::ArchetypeDictionary |
            ContentKind::MaterialLibrary | ContentKind::AiPatternDictionary | ContentKind::UiDocument => Self::ListFile,
            _ => Self::RawFile,
        }
    }
    fn entry_kind(self) -> EntryKind { match self { Self::Directory => EntryKind::Directory, Self::ListFile => EntryKind::Resource, Self::RawFile => EntryKind::File } }
    fn as_str(self) -> &'static str { match self { Self::RawFile => "raw_file", Self::Directory => "directory", Self::ListFile => "listfile" } }
}

#[derive(Debug, Clone, Default)]
pub struct ResourceLayout {
    pub resource_version: u32,
    pub virtual_size: u64,
    pub physical_size: u64,
    pub virtual_chunk_size: u32,
    pub physical_chunk_size: u32,
    pub virtual_chunk_count: u32,
    pub physical_chunk_count: u32,
}

#[derive(Debug, Clone)]
pub struct PackageEntry {
    pub index: u32,
    pub name: String,
    pub path: String,
    pub parent_index: u32,
    pub first_child_index: u32,
    pub child_count: u32,
    pub entry_kind: EntryKind,
    pub content_kind: ContentKind,
    pub storage_class: StorageClass,
    pub data_sector: u64,
    pub byte_offset: u64,
    pub stored_size: u64,
    pub decoded_size: u64,
    pub compression: CompressionKind,
    pub flags: u16,
    pub hash: [u8; 32],
    pub resource_layout: ResourceLayout,
}

#[derive(Debug, Clone)]
pub struct ParsedPackage {
    pub entries: Vec<PackageEntry>,
    pub total_size: u64,
    pub data_offset: u64,
    pub data_size: u64,
    pub index_hash: [u8; 32],
    pub data_hash: [u8; 32],
}

struct SourceRecord {
    path: String,
    name: String,
    payload_index: usize,
    content_kind: ContentKind,
    storage_class: StorageClass,
    compression: CompressionKind,
    stored_size: u64,
    decoded_size: u64,
    hash: [u8; 32],
}

#[derive(Clone)]
struct BuildEntry {
    name: String,
    path: String,
    parent_index: u32,
    first_child_index: u32,
    child_count: u32,
    source_index: Option<usize>,
    content_kind: ContentKind,
    storage_class: StorageClass,
    compression: CompressionKind,
    data_sector: u64,
    stored_size: u64,
    decoded_size: u64,
    flags: u16,
    hash: [u8; 32],
    resource_layout: ResourceLayout,
}

pub fn pack_sources(sources: &[SourceFile], output: &Path, compress: bool) -> Result<(), String> {
    if sources.is_empty() { return Err("NEPAK pack requires at least one source file".to_owned()); }

    let mut used = BTreeSet::new();
    let mut decoded_payloads = Vec::<Vec<u8>>::new();
    let mut stored_payloads = Vec::<Vec<u8>>::new();
    let mut records = Vec::<SourceRecord>::new();

    for source in sources {
        validate_package_path(&source.package_path)?;
        if !used.insert(source.package_path.to_ascii_lowercase()) { return Err(format!("duplicate NEPAK entry path '{}'", source.package_path)); }
        let decoded = fs::read(&source.disk_path).map_err(|e| format!("read '{}' failed: {e}", source.disk_path.display()))?;
        let maybe_deflated = if compress { Some(deflate(&decoded)?) } else { None };
        let (compression, stored) = match maybe_deflated {
            Some(deflated) if deflated.len() < decoded.len() => (CompressionKind::Deflate, deflated),
            _ => (CompressionKind::None, decoded.clone()),
        };
        let content_kind = ContentKind::for_path(&source.package_path);
        let storage_class = StorageClass::for_content(content_kind, false);
        let payload_index = decoded_payloads.len();
        let (_, name) = split_parent(&source.package_path);
        records.push(SourceRecord {
            path: source.package_path.clone(), name, payload_index, content_kind, storage_class, compression,
            stored_size: stored.len() as u64, decoded_size: decoded.len() as u64, hash: *blake3::hash(&decoded).as_bytes(),
        });
        decoded_payloads.push(decoded);
        stored_payloads.push(stored);
    }

    let mut entries = build_entries(&records)?;
    let mut names = Vec::new();
    let mut name_offsets = Vec::with_capacity(entries.len());
    for entry in &entries { name_offsets.push(push_name(&mut names, &entry.name)?); }

    let table_len = entries.len() * ENTRY_LEN;
    let data_offset = align_u64((HEADER_LEN + table_len + names.len()) as u64, SECTOR_SIZE);
    let mut data = Vec::new();
    for entry in &mut entries {
        let Some(source_index) = entry.source_index else { continue; };
        let absolute = align_u64(data_offset + data.len() as u64, SECTOR_SIZE);
        data.resize((absolute - data_offset) as usize, 0);
        let record = &records[source_index];
        let stored = &stored_payloads[record.payload_index];
        entry.data_sector = absolute / SECTOR_SIZE;
        entry.stored_size = stored.len() as u64;
        entry.decoded_size = decoded_payloads[record.payload_index].len() as u64;
        entry.resource_layout = layout(entry.decoded_size, entry.stored_size);
        data.extend_from_slice(stored);
    }

    let mut index = Vec::with_capacity(table_len + names.len());
    for (i, entry) in entries.iter().enumerate() {
        write_u32_vec(&mut index, name_offsets[i]);
        write_u16_vec(&mut index, entry.name.as_bytes().len() as u16);
        write_u16_vec(&mut index, entry.storage_class.entry_kind() as u16);
        write_u32_vec(&mut index, entry.parent_index);
        write_u32_vec(&mut index, entry.first_child_index);
        write_u32_vec(&mut index, entry.child_count);
        write_u16_vec(&mut index, entry.content_kind as u16);
        write_u16_vec(&mut index, entry.storage_class as u16);
        write_u16_vec(&mut index, entry.compression as u16);
        write_u16_vec(&mut index, entry.flags);
        write_u64_vec(&mut index, entry.data_sector);
        write_u64_vec(&mut index, entry.stored_size);
        write_u64_vec(&mut index, entry.decoded_size);
        index.extend_from_slice(&entry.hash);
        write_u32_vec(&mut index, entry.resource_layout.resource_version);
        write_u64_vec(&mut index, entry.resource_layout.virtual_size);
        write_u64_vec(&mut index, entry.resource_layout.physical_size);
        write_u32_vec(&mut index, entry.resource_layout.virtual_chunk_size);
        write_u32_vec(&mut index, entry.resource_layout.physical_chunk_size);
        write_u32_vec(&mut index, entry.resource_layout.virtual_chunk_count);
        write_u32_vec(&mut index, entry.resource_layout.physical_chunk_count);
        index.resize((i + 1) * ENTRY_LEN, 0);
    }
    index.extend_from_slice(&names);

    let mut out = vec![0u8; HEADER_LEN];
    out[0..8].copy_from_slice(MAGIC);
    write_u16(&mut out, 8, VERSION_MAJOR);
    write_u16(&mut out, 10, VERSION_MINOR);
    write_u32(&mut out, 16, HEADER_LEN as u32);
    out[20] = ENDIAN_LITTLE;
    out[21] = SECTOR_SHIFT;
    write_u16(&mut out, 22, ENTRY_LEN as u16);
    write_u32(&mut out, 24, entries.len() as u32);
    write_u32(&mut out, 28, names.len() as u32);
    write_u64(&mut out, 32, HEADER_LEN as u64);
    write_u64(&mut out, 40, (HEADER_LEN + table_len) as u64);
    write_u64(&mut out, 48, data_offset);
    write_u64(&mut out, 56, data.len() as u64);
    out[64..96].copy_from_slice(blake3::hash(&index).as_bytes());
    out[96..128].copy_from_slice(blake3::hash(&data).as_bytes());
    out.extend_from_slice(&index);
    out.resize(data_offset as usize, 0);
    out.extend_from_slice(&data);

    if let Some(parent) = output.parent() { if !parent.as_os_str().is_empty() { fs::create_dir_all(parent).map_err(|e| format!("create parent '{}' failed: {e}", parent.display()))?; } }
    fs::write(output, out).map_err(|e| format!("write '{}' failed: {e}", output.display()))
}

pub fn parse(bytes: &[u8]) -> Result<ParsedPackage, String> {
    if bytes.len() < HEADER_LEN { return Err(format!("NEPAK header too small: {}", bytes.len())); }
    if bytes.get(0..8) != Some(MAGIC) { return Err("NEPAK magic mismatch".to_owned()); }
    let major = read_u16(bytes, 8)?;
    let minor = read_u16(bytes, 10)?;
    if major != VERSION_MAJOR || minor != VERSION_MINOR { return Err(format!("unsupported NEPAK version {major}.{minor}")); }
    if read_u32(bytes, 16)? as usize != HEADER_LEN { return Err("unsupported NEPAK header size".to_owned()); }
    if bytes[20] != ENDIAN_LITTLE { return Err(format!("unsupported NEPAK endian marker {}", bytes[20])); }
    if bytes[21] != SECTOR_SHIFT { return Err(format!("unsupported NEPAK sector shift {}", bytes[21])); }
    if read_u16(bytes, 22)? as usize != ENTRY_LEN { return Err("unsupported NEPAK entry size".to_owned()); }

    let entry_count = read_u32(bytes, 24)? as usize;
    let name_size = read_u32(bytes, 28)? as usize;
    let index_offset = read_u64(bytes, 32)? as usize;
    let name_offset = read_u64(bytes, 40)? as usize;
    let data_offset = read_u64(bytes, 48)? as usize;
    let data_size = read_u64(bytes, 56)? as usize;
    let index_hash = read_hash32(bytes, 64)?;
    let data_hash = read_hash32(bytes, 96)?;
    if entry_count == 0 { return Err("NEPAK central directory has no root entry".to_owned()); }
    let table_len = entry_count.checked_mul(ENTRY_LEN).ok_or("NEPAK entry table overflow")?;
    if name_offset != index_offset + table_len { return Err("NEPAK name table offset does not follow entry table".to_owned()); }
    let index_len = table_len.checked_add(name_size).ok_or("NEPAK central directory overflow")?;
    let index_bytes = checked_slice(bytes, index_offset, index_len, "NEPAK central directory outside file")?;
    let names = checked_slice(bytes, name_offset, name_size, "NEPAK name table outside file")?;
    let data = checked_slice(bytes, data_offset, data_size, "NEPAK data section outside file")?;
    if data_offset as u64 % SECTOR_SIZE != 0 { return Err("NEPAK data section is not sector-aligned".to_owned()); }
    if blake3::hash(index_bytes).as_bytes() != &index_hash { return Err("NEPAK central directory hash mismatch".to_owned()); }
    if blake3::hash(data).as_bytes() != &data_hash { return Err("NEPAK data section hash mismatch".to_owned()); }

    let mut entries = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let o = i * ENTRY_LEN;
        let name_pos = read_u32(index_bytes, o)?;
        let name_len = read_u16(index_bytes, o + 4)?;
        let name = read_name(names, name_pos, name_len)?;
        let entry_kind = EntryKind::from_u16(read_u16(index_bytes, o + 6)?)?;
        let parent_index = read_u32(index_bytes, o + 8)?;
        let first_child_index = read_u32(index_bytes, o + 12)?;
        let child_count = read_u32(index_bytes, o + 16)?;
        let content_kind = ContentKind::from_u16(read_u16(index_bytes, o + 20)?)?;
        let storage_class = StorageClass::from_u16(read_u16(index_bytes, o + 22)?)?;
        let compression = CompressionKind::from_u16(read_u16(index_bytes, o + 24)?)?;
        let flags = read_u16(index_bytes, o + 26)?;
        let data_sector = read_u64(index_bytes, o + 28)?;
        let stored_size = read_u64(index_bytes, o + 36)?;
        let decoded_size = read_u64(index_bytes, o + 44)?;
        let hash = read_hash32(index_bytes, o + 52)?;
        let resource_layout = ResourceLayout {
            resource_version: read_u32(index_bytes, o + 84)?,
            virtual_size: read_u64(index_bytes, o + 88)?,
            physical_size: read_u64(index_bytes, o + 96)?,
            virtual_chunk_size: read_u32(index_bytes, o + 104)?,
            physical_chunk_size: read_u32(index_bytes, o + 108)?,
            virtual_chunk_count: read_u32(index_bytes, o + 112)?,
            physical_chunk_count: read_u32(index_bytes, o + 116)?,
        };
        entries.push(PackageEntry {
            index: i as u32, name, path: String::new(), parent_index, first_child_index, child_count,
            entry_kind, content_kind, storage_class, data_sector, byte_offset: data_sector * SECTOR_SIZE,
            stored_size, decoded_size, compression, flags, hash, resource_layout,
        });
    }
    validate_and_materialize_paths(&mut entries, data_offset as u64, data_size as u64)?;
    Ok(ParsedPackage { entries, total_size: bytes.len() as u64, data_offset: data_offset as u64, data_size: data_size as u64, index_hash, data_hash })
}

pub fn inspect_json(bytes: &[u8], input: &Path) -> Result<serde_json::Value, String> {
    let parsed = parse(bytes)?;
    Ok(json!({
        "schema": "northstar.nepak.inspect.rpf_like.v1",
        "ok": true,
        "container": "NEPAK VFS package",
        "format": "nepak",
        "layout": "header_entry_table_name_table_sector_data",
        "version": "1.0",
        "file": input.to_string_lossy().replace('\\', "/"),
        "central_directory": { "entry_count": parsed.entries.len(), "entry_size": ENTRY_LEN, "sector_size": SECTOR_SIZE, "hash_blake3": hex32(&parsed.index_hash) },
        "data": { "offset": parsed.data_offset, "size": parsed.data_size, "hash_blake3": hex32(&parsed.data_hash) },
        "totals": totals_json(&parsed),
        "entries": parsed.entries.iter().map(entry_json).collect::<Vec<_>>(),
    }))
}

pub fn manifest_json(bytes: &[u8]) -> Result<serde_json::Value, String> { Ok(generated_manifest_json(&parse(bytes)?)) }

pub fn mount_test_json(bytes: &[u8], input: &Path) -> Result<serde_json::Value, String> {
    let parsed = parse(bytes)?;
    let nested = parsed.entries.iter().filter(|e| e.content_kind == ContentKind::VfsPackage).map(|e| json!({
        "path": e.path, "index": e.index, "mount_policy": "descriptor_required", "byte_offset": e.byte_offset, "stored_size": e.stored_size, "decoded_size": e.decoded_size
    })).collect::<Vec<_>>();
    Ok(json!({
        "schema": "northstar.nepak.mount_test.rpf_like.v1", "ok": true,
        "file": input.to_string_lossy().replace('\\', "/"), "root_layer": "/", "nested_layers": nested,
        "diagnostics": [
            "central directory is authoritative; mount-test does not infer child layers from filename alone",
            "nested .nepak entries require package descriptor/profile routing before they become VFS layers"
        ]
    }))
}

pub fn diff_json(old_bytes: &[u8], new_bytes: &[u8]) -> Result<serde_json::Value, String> {
    let old = parse(old_bytes)?;
    let new = parse(new_bytes)?;
    let old_map = file_map(&old);
    let new_map = file_map(&new);
    let mut added = Vec::new(); let mut removed = Vec::new(); let mut changed = Vec::new(); let mut unchanged = Vec::new();
    for path in old_map.keys() { if !new_map.contains_key(path) { removed.push(path.clone()); } }
    for (path, entry) in &new_map {
        match old_map.get(path) {
            None => added.push(path.clone()),
            Some(old_entry) if old_entry.hash != entry.hash || old_entry.decoded_size != entry.decoded_size => changed.push(json!({
                "path": path, "old_hash": hex32(&old_entry.hash), "new_hash": hex32(&entry.hash),
                "old_decoded_size": old_entry.decoded_size, "new_decoded_size": entry.decoded_size
            })),
            Some(_) => unchanged.push(path.clone()),
        }
    }
    Ok(json!({ "schema": "northstar.nepak.diff.rpf_like.v1", "ok": true,
        "summary": { "added": added.len(), "removed": removed.len(), "changed": changed.len(), "unchanged": unchanged.len() },
        "added": added, "removed": removed, "changed": changed, "unchanged": unchanged }))
}

pub fn list_paths(bytes: &[u8]) -> Result<Vec<String>, String> {
    Ok(parse(bytes)?.entries.into_iter().filter(|e| e.entry_kind != EntryKind::Directory).map(|e| e.path).collect())
}

pub fn validate_bytes(bytes: &[u8]) -> Result<usize, String> {
    let parsed = parse(bytes)?;
    for entry in parsed.entries.iter().filter(|e| e.entry_kind != EntryKind::Directory) {
        let raw = entry_payload(bytes, &parsed, entry)?;
        if raw.len() as u64 != entry.decoded_size { return Err(format!("NEPAK entry '{}' decoded size mismatch", entry.path)); }
        if blake3::hash(&raw).as_bytes() != &entry.hash { return Err(format!("NEPAK entry '{}' hash mismatch", entry.path)); }
    }
    Ok(parsed.entries.iter().filter(|e| e.entry_kind != EntryKind::Directory).count())
}

pub fn extract_to(bytes: &[u8], output_root: &Path, filter: Option<&str>, overwrite: bool) -> Result<usize, String> {
    let parsed = parse(bytes)?;
    let mut count = 0;
    for entry in parsed.entries.iter().filter(|e| e.entry_kind != EntryKind::Directory) {
        if let Some(filter) = filter { if entry.path != filter { continue; } }
        let raw = entry_payload(bytes, &parsed, entry)?;
        let out = safe_output_path(output_root, &entry.path)?;
        if out.exists() && !overwrite { return Err(format!("output '{}' exists; use --overwrite", out.display())); }
        if let Some(parent) = out.parent() { fs::create_dir_all(parent).map_err(|e| format!("create parent '{}' failed: {e}", parent.display()))?; }
        fs::write(&out, raw).map_err(|e| format!("write '{}' failed: {e}", out.display()))?;
        count += 1;
    }
    Ok(count)
}

pub fn read_entry_bytes(bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let parsed = parse(bytes)?;
    for entry in &parsed.entries {
        if entry.entry_kind != EntryKind::Directory && entry.path == name {
            return entry_payload(bytes, &parsed, entry);
        }
    }
    Err(String::from("NEPAK entry not found"))
}

fn build_entries(records: &[SourceRecord]) -> Result<Vec<BuildEntry>, String> {
    let mut child_dirs = BTreeMap::<String, BTreeSet<String>>::new();
    let mut child_files = BTreeMap::<String, Vec<usize>>::new();
    for (idx, record) in records.iter().enumerate() {
        let (dir, _) = split_parent(&record.path);
        child_files.entry(dir.clone()).or_default().push(idx);
        let mut current = String::new();
        for segment in dir.split('/').filter(|s| !s.is_empty()) {
            let parent = current.clone();
            current = if current.is_empty() { segment.to_owned() } else { format!("{current}/{segment}") };
            child_dirs.entry(parent).or_default().insert(current.clone());
        }
    }
    let mut entries = vec![BuildEntry::dir(".".to_owned(), String::new(), ROOT_PARENT)];
    let mut dir_to_index = BTreeMap::from([(String::new(), 0u32)]);
    let mut queue = VecDeque::from([String::new()]);
    while let Some(dir_path) = queue.pop_front() {
        let parent_index = *dir_to_index.get(&dir_path).ok_or("internal NEPAK directory index missing")?;
        let first_child_index = entries.len() as u32;
        let mut child_count = 0u32;
        if let Some(dirs) = child_dirs.get(&dir_path) {
            for child in dirs {
                let index = entries.len() as u32;
                entries.push(BuildEntry::dir(basename(child).to_owned(), child.clone(), parent_index));
                dir_to_index.insert(child.clone(), index);
                queue.push_back(child.clone());
                child_count += 1;
            }
        }
        if let Some(files) = child_files.get_mut(&dir_path) {
            files.sort_by(|a, b| records[*a].path.cmp(&records[*b].path));
            for idx in files.iter().copied() {
                let rec = &records[idx];
                entries.push(BuildEntry::file(rec.name.clone(), rec.path.clone(), parent_index, idx, rec));
                child_count += 1;
            }
        }
        let parent = entries.get_mut(parent_index as usize).ok_or("internal NEPAK parent index missing")?;
        parent.first_child_index = if child_count == 0 { 0 } else { first_child_index };
        parent.child_count = child_count;
    }
    Ok(entries)
}

impl BuildEntry {
    fn dir(name: String, path: String, parent_index: u32) -> Self {
        Self { name, path, parent_index, first_child_index: 0, child_count: 0, source_index: None,
            content_kind: ContentKind::OpaqueFile, storage_class: StorageClass::Directory, compression: CompressionKind::None,
            data_sector: 0, stored_size: 0, decoded_size: 0, flags: 0, hash: [0; 32], resource_layout: ResourceLayout::default() }
    }
    fn file(name: String, path: String, parent_index: u32, source_index: usize, rec: &SourceRecord) -> Self {
        Self { name, path, parent_index, first_child_index: 0, child_count: 0, source_index: Some(source_index),
            content_kind: rec.content_kind, storage_class: rec.storage_class, compression: rec.compression,
            data_sector: 0, stored_size: rec.stored_size, decoded_size: rec.decoded_size, flags: 0, hash: rec.hash,
            resource_layout: layout(rec.decoded_size, rec.stored_size) }
    }
}

fn validate_and_materialize_paths(entries: &mut [PackageEntry], data_offset: u64, data_size: u64) -> Result<(), String> {
    if entries[0].entry_kind != EntryKind::Directory || entries[0].parent_index != ROOT_PARENT { return Err("NEPAK root entry must be directory with ROOT_PARENT".to_owned()); }
    for i in 0..entries.len() {
        if i != 0 && entries[i].parent_index as usize >= entries.len() { return Err(format!("NEPAK entry {i} parent index outside table")); }
        if entries[i].entry_kind != entries[i].storage_class.entry_kind() { return Err(format!("NEPAK entry {i} kind/storage_class mismatch")); }
        if entries[i].entry_kind == EntryKind::Directory {
            let end = (entries[i].first_child_index as usize).checked_add(entries[i].child_count as usize).ok_or("NEPAK child range overflow")?;
            if entries[i].child_count > 0 && (entries[i].first_child_index as usize == 0 || end > entries.len()) { return Err(format!("NEPAK directory entry '{}' child range outside table", entries[i].name)); }
            for child in entries[i].first_child_index as usize..end { if entries[child].parent_index != i as u32 { return Err(format!("NEPAK directory '{}' child range contains non-child {child}", entries[i].name)); } }
        } else {
            if entries[i].byte_offset < data_offset { return Err(format!("NEPAK entry '{}' data sector before data section", entries[i].name)); }
            let rel = (entries[i].byte_offset - data_offset) as usize;
            let end = rel.checked_add(entries[i].stored_size as usize).ok_or("NEPAK payload range overflow")?;
            if end > data_size as usize { return Err(format!("NEPAK entry '{}' payload outside data section", entries[i].name)); }
        }
    }
    let mut memo = vec![None::<String>; entries.len()];
    let mut seen = BTreeSet::new();
    for i in 0..entries.len() {
        entries[i].path = compute_path(i, entries, &mut memo, &mut BTreeSet::new())?;
        if entries[i].entry_kind != EntryKind::Directory {
            validate_package_path(&entries[i].path)?;
            if !seen.insert(entries[i].path.to_ascii_lowercase()) { return Err(format!("duplicate NEPAK entry path '{}'", entries[i].path)); }
            let expected = ContentKind::for_path(&entries[i].path);
            if entries[i].content_kind != expected { return Err(format!("NEPAK entry '{}' extension/content_kind mismatch: expected {}, got {}", entries[i].path, expected.as_str(), entries[i].content_kind.as_str())); }
        }
    }
    Ok(())
}

fn compute_path(index: usize, entries: &[PackageEntry], memo: &mut [Option<String>], visiting: &mut BTreeSet<usize>) -> Result<String, String> {
    if let Some(path) = &memo[index] { return Ok(path.clone()); }
    if !visiting.insert(index) { return Err(format!("NEPAK directory cycle at entry {index}")); }
    let path = if index == 0 { String::new() } else {
        let parent = entries[index].parent_index as usize;
        let parent_path = compute_path(parent, entries, memo, visiting)?;
        if parent_path.is_empty() { entries[index].name.clone() } else { format!("{parent_path}/{}", entries[index].name) }
    };
    visiting.remove(&index);
    memo[index] = Some(path.clone());
    Ok(path)
}

fn entry_payload(bytes: &[u8], _parsed: &ParsedPackage, entry: &PackageEntry) -> Result<Vec<u8>, String> {
    let stored = checked_slice(bytes, entry.byte_offset as usize, entry.stored_size as usize, "NEPAK entry payload outside file")?;
    match entry.compression { CompressionKind::None => Ok(stored.to_vec()), CompressionKind::Deflate => inflate(stored) }
}

fn file_map(parsed: &ParsedPackage) -> BTreeMap<String, &PackageEntry> {
    parsed.entries.iter().filter(|e| e.entry_kind != EntryKind::Directory).map(|e| (e.path.clone(), e)).collect()
}

fn generated_manifest_json(parsed: &ParsedPackage) -> serde_json::Value {
    json!({
        "schema": "northstar.nepak.manifest.rpf_like.v1",
        "source": "generated_from_central_directory",
        "package": { "format": "nepak", "version": "1.0", "layout": "header_entry_table_name_table_sector_data", "package_id": "package", "mount": "/game", "created_by": "northstar-nepak-manager", "semantics": "vfs_package_only" },
        "profiles": [
            { "id": "pc.vulkan", "platform": "pc", "endianness": "little", "gpu_asset_tier": "desktop_bc", "requires": ["assets.listfile.nef8", "assets.container.nepak"], "optional": ["render.texture.bc7", "materials.graph.resolve"] },
            { "id": "pc.dx12", "platform": "pc", "endianness": "little", "gpu_asset_tier": "desktop_bc", "requires": ["assets.listfile.nef8", "assets.container.nepak"], "optional": ["render.texture.bc7", "materials.graph.resolve"] },
            { "id": "editor", "platform": "editor", "endianness": "little", "gpu_asset_tier": "tooling", "requires": ["assets.container.nepak", "assets.container.nepak.inspect"], "optional": ["assets.container.nepak.writer", "assets.browser.mutation"] }
        ],
        "entries": parsed.entries.iter().filter(|e| e.entry_kind != EntryKind::Directory).map(|e| json!({
            "path": e.path, "index": e.index, "entry_kind": e.entry_kind.as_str(), "content_kind": e.content_kind.as_str(),
            "storage_class": e.storage_class.as_str(), "ref_style": ref_style_for(e), "compression": e.compression.as_str(),
            "data_sector": e.data_sector, "byte_offset": e.byte_offset, "decoded_size": e.decoded_size, "stored_size": e.stored_size,
            "hash": format!("blake3:{}", hex32(&e.hash)), "resource_layout": resource_layout_json(&e.resource_layout)
        })).collect::<Vec<_>>()
    })
}

fn entry_json(entry: &PackageEntry) -> serde_json::Value {
    json!({
        "index": entry.index, "name": entry.name, "path": entry.path,
        "parent_index": if entry.parent_index == ROOT_PARENT { serde_json::Value::Null } else { json!(entry.parent_index) },
        "first_child_index": entry.first_child_index, "child_count": entry.child_count,
        "entry_kind": entry.entry_kind.as_str(), "content_kind": entry.content_kind.as_str(), "storage_class": entry.storage_class.as_str(),
        "data_sector": entry.data_sector, "byte_offset": entry.byte_offset, "stored_size": entry.stored_size, "decoded_size": entry.decoded_size,
        "compression": entry.compression.as_str(), "flags": entry.flags, "hash_blake3": hex32(&entry.hash),
        "resource_layout": resource_layout_json(&entry.resource_layout)
    })
}

fn totals_json(parsed: &ParsedPackage) -> serde_json::Value {
    let directories = parsed.entries.iter().filter(|e| e.entry_kind == EntryKind::Directory).count();
    let files = parsed.entries.iter().filter(|e| e.entry_kind == EntryKind::File).count();
    let resources = parsed.entries.iter().filter(|e| e.entry_kind == EntryKind::Resource).count();
    let total_stored: u64 = parsed.entries.iter().map(|e| e.stored_size).sum();
    let total_decoded: u64 = parsed.entries.iter().map(|e| e.decoded_size).sum();
    let total_virtual: u64 = parsed.entries.iter().map(|e| e.resource_layout.virtual_size).sum();
    let total_physical: u64 = parsed.entries.iter().map(|e| e.resource_layout.physical_size).sum();
    let total_virtual_chunks: u64 = parsed.entries.iter().map(|e| e.resource_layout.virtual_chunk_count as u64).sum();
    let total_physical_chunks: u64 = parsed.entries.iter().map(|e| e.resource_layout.physical_chunk_count as u64).sum();
    json!({ "total_size": parsed.total_size, "total_stored": total_stored, "total_decoded": total_decoded,
        "total_virtual_size": total_virtual, "total_physical_size": total_physical,
        "total_virtual_chunk_count": total_virtual_chunks, "total_physical_chunk_count": total_physical_chunks,
        "entry_count": parsed.entries.len(), "directory_count": directories, "file_count": files, "resource_count": resources, "unknown_count": 0 })
}

fn resource_layout_json(layout: &ResourceLayout) -> serde_json::Value {
    json!({ "resource_version": layout.resource_version, "virtual_size": layout.virtual_size, "physical_size": layout.physical_size,
        "virtual_chunk_size": layout.virtual_chunk_size, "physical_chunk_size": layout.physical_chunk_size,
        "virtual_chunk_count": layout.virtual_chunk_count, "physical_chunk_count": layout.physical_chunk_count })
}

fn ref_style_for(entry: &PackageEntry) -> String { if entry.storage_class == StorageClass::ListFile { format!("{}@<entry>", entry.path) } else { entry.path.clone() } }
fn layout(decoded: u64, stored: u64) -> ResourceLayout { ResourceLayout { resource_version: 0, virtual_size: decoded, physical_size: stored, virtual_chunk_size: decoded.min(u32::MAX as u64) as u32, physical_chunk_size: stored.min(u32::MAX as u64) as u32, virtual_chunk_count: if decoded == 0 { 0 } else { 1 }, physical_chunk_count: if stored == 0 { 0 } else { 1 } } }
fn split_parent(path: &str) -> (String, String) { path.rsplit_once('/').map(|(d, n)| (d.to_owned(), n.to_owned())).unwrap_or_else(|| (String::new(), path.to_owned())) }
fn basename(path: &str) -> &str { path.rsplit_once('/').map(|(_, name)| name).unwrap_or(path) }
fn lower_extension(path: &str) -> Option<String> { path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).filter(|ext| !ext.contains('/') && !ext.contains('\\') && !ext.is_empty()) }
fn push_name(names: &mut Vec<u8>, value: &str) -> Result<u32, String> { if value.len() > u16::MAX as usize { return Err(format!("NEPAK name too long: {value}")); } let off = names.len(); if off > u32::MAX as usize { return Err("NEPAK name table exceeds u32 offset range".to_owned()); } names.extend_from_slice(value.as_bytes()); Ok(off as u32) }
fn read_name(names: &[u8], offset: u32, len: u16) -> Result<String, String> { let start = offset as usize; let end = start.checked_add(len as usize).ok_or("NEPAK name range overflow")?; let bytes = names.get(start..end).ok_or("NEPAK name range outside table")?; String::from_utf8(bytes.to_vec()).map_err(|e| format!("NEPAK name is not UTF-8: {e}")) }
fn deflate(bytes: &[u8]) -> Result<Vec<u8>, String> { let mut e = DeflateEncoder::new(Vec::new(), Compression::default()); e.write_all(bytes).map_err(|e| e.to_string())?; e.finish().map_err(|e| e.to_string()) }
fn inflate(bytes: &[u8]) -> Result<Vec<u8>, String> { let mut d = DeflateDecoder::new(bytes); let mut out = Vec::new(); d.read_to_end(&mut out).map_err(|e| format!("deflate decode failed: {e}"))?; Ok(out) }
fn checked_slice<'a>(bytes: &'a [u8], offset: usize, len: usize, err: &str) -> Result<&'a [u8], String> { let end = offset.checked_add(len).ok_or(err.to_owned())?; bytes.get(offset..end).ok_or_else(|| err.to_owned()) }
fn align_u64(value: u64, alignment: u64) -> u64 { (value + alignment - 1) & !(alignment - 1) }
fn hex32(v: &[u8; 32]) -> String { v.iter().map(|b| format!("{b:02x}")).collect() }
fn write_u16(b: &mut [u8], o: usize, v: u16) { b[o..o + 2].copy_from_slice(&v.to_le_bytes()); }
fn write_u32(b: &mut [u8], o: usize, v: u32) { b[o..o + 4].copy_from_slice(&v.to_le_bytes()); }
fn write_u64(b: &mut [u8], o: usize, v: u64) { b[o..o + 8].copy_from_slice(&v.to_le_bytes()); }
fn write_u16_vec(b: &mut Vec<u8>, v: u16) { b.extend_from_slice(&v.to_le_bytes()); }
fn write_u32_vec(b: &mut Vec<u8>, v: u32) { b.extend_from_slice(&v.to_le_bytes()); }
fn write_u64_vec(b: &mut Vec<u8>, v: u64) { b.extend_from_slice(&v.to_le_bytes()); }
fn read_u16(b: &[u8], o: usize) -> Result<u16, String> { let s = b.get(o..o + 2).ok_or("truncated u16")?; Ok(u16::from_le_bytes(s.try_into().unwrap())) }
fn read_u32(b: &[u8], o: usize) -> Result<u32, String> { let s = b.get(o..o + 4).ok_or("truncated u32")?; Ok(u32::from_le_bytes(s.try_into().unwrap())) }
fn read_u64(b: &[u8], o: usize) -> Result<u64, String> { let s = b.get(o..o + 8).ok_or("truncated u64")?; Ok(u64::from_le_bytes(s.try_into().unwrap())) }
fn read_hash32(b: &[u8], o: usize) -> Result<[u8; 32], String> { let s = b.get(o..o + 32).ok_or("truncated hash32")?; let mut out = [0u8; 32]; out.copy_from_slice(s); Ok(out) }

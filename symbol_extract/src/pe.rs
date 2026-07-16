#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRecord {
    pub name: String,
    pub rva: u32,
    pub source: SymbolSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolSource {
    Export,
    Coff,
}

impl SymbolSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Coff => "coff",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Section {
    virtual_address: u32,
    virtual_size: u32,
    raw_pointer: u32,
    raw_size: u32,
}

struct PeImage<'a> {
    bytes: &'a [u8],
    optional_offset: usize,
    optional_size: usize,
    symbol_table_offset: usize,
    symbol_count: usize,
    sections: Vec<Section>,
}

pub fn parse_symbols(bytes: &[u8]) -> Result<Vec<SymbolRecord>, String> {
    let mut out = parse_export_symbols(bytes)?;
    out.extend(parse_coff_symbols_from_image(bytes)?);
    Ok(out)
}

pub fn parse_export_symbols(bytes: &[u8]) -> Result<Vec<SymbolRecord>, String> {
    let image = PeImage::parse(bytes)?;
    let mut out = Vec::new();
    parse_exports(&image, &mut out)?;
    Ok(out)
}

pub fn parse_coff_symbols_from_image(bytes: &[u8]) -> Result<Vec<SymbolRecord>, String> {
    let image = PeImage::parse(bytes)?;
    let mut out = Vec::new();
    parse_coff_symbols(image.bytes, image.symbol_table_offset, image.symbol_count, &mut out)?;
    Ok(out)
}

impl<'a> PeImage<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
            return Err("not a PE image: missing MZ header".to_string());
        }

        let pe_offset = read_u32(bytes, 0x3c)? as usize;
        if pe_offset + 24 > bytes.len() || &bytes[pe_offset..pe_offset + 4] != b"PE\0\0" {
            return Err("not a PE image: missing PE signature".to_string());
        }

        let section_count = read_u16(bytes, pe_offset + 6)? as usize;
        let symbol_table_offset = read_u32(bytes, pe_offset + 12)? as usize;
        let symbol_count = read_u32(bytes, pe_offset + 16)? as usize;
        let optional_size = read_u16(bytes, pe_offset + 20)? as usize;
        let optional_offset = pe_offset + 24;
        let section_offset = optional_offset + optional_size;

        if section_offset + section_count.saturating_mul(40) > bytes.len() {
            return Err("PE section table is outside file".to_string());
        }

        let mut sections = Vec::with_capacity(section_count);
        for i in 0..section_count {
            let base = section_offset + i * 40;
            sections.push(Section {
                virtual_size: read_u32(bytes, base + 8)?,
                virtual_address: read_u32(bytes, base + 12)?,
                raw_size: read_u32(bytes, base + 16)?,
                raw_pointer: read_u32(bytes, base + 20)?,
            });
        }

        Ok(Self { bytes, optional_offset, optional_size, symbol_table_offset, symbol_count, sections })
    }
}

fn parse_exports(image: &PeImage<'_>, out: &mut Vec<SymbolRecord>) -> Result<(), String> {
    let bytes = image.bytes;
    if image.optional_size < 96 || image.optional_offset + 2 > bytes.len() {
        return Ok(());
    }

    let magic = read_u16(bytes, image.optional_offset)?;
    let data_dir_offset = match magic {
        0x10b => image.optional_offset + 96,
        0x20b => image.optional_offset + 112,
        _ => return Ok(()),
    };

    if data_dir_offset + 8 > image.optional_offset + image.optional_size || data_dir_offset + 8 > bytes.len() {
        return Ok(());
    }

    let export_rva = read_u32(bytes, data_dir_offset)?;
    if export_rva == 0 {
        return Ok(());
    }

    let Some(export_offset) = rva_to_offset(&image.sections, export_rva) else { return Ok(()); };
    if export_offset + 40 > bytes.len() {
        return Ok(());
    }

    let number_of_functions = read_u32(bytes, export_offset + 20)? as usize;
    let number_of_names = read_u32(bytes, export_offset + 24)? as usize;
    let Some(functions_offset) = rva_to_offset(&image.sections, read_u32(bytes, export_offset + 28)?) else { return Ok(()); };
    let Some(names_offset) = rva_to_offset(&image.sections, read_u32(bytes, export_offset + 32)?) else { return Ok(()); };
    let Some(ordinals_offset) = rva_to_offset(&image.sections, read_u32(bytes, export_offset + 36)?) else { return Ok(()); };

    for i in 0..number_of_names {
        let name_rva = read_u32(bytes, names_offset + i * 4)?;
        let Some(name_offset) = rva_to_offset(&image.sections, name_rva) else { continue; };
        let name = read_c_string(bytes, name_offset);
        if name.is_empty() {
            continue;
        }
        let ordinal = read_u16(bytes, ordinals_offset + i * 2)? as usize;
        if ordinal >= number_of_functions {
            continue;
        }
        let rva = read_u32(bytes, functions_offset + ordinal * 4)?;
        out.push(SymbolRecord { name, rva, source: SymbolSource::Export });
    }

    Ok(())
}

fn parse_coff_symbols(bytes: &[u8], symbol_table_offset: usize, symbol_count: usize, out: &mut Vec<SymbolRecord>) -> Result<(), String> {
    if symbol_table_offset == 0 || symbol_count == 0 {
        return Ok(());
    }

    let table_bytes = symbol_count.saturating_mul(18);
    if symbol_table_offset + table_bytes > bytes.len() {
        return Ok(());
    }

    let string_table_offset = symbol_table_offset + table_bytes;
    let string_table_size = if string_table_offset + 4 <= bytes.len() {
        read_u32(bytes, string_table_offset).unwrap_or(0) as usize
    } else {
        0
    };

    let mut i = 0usize;
    while i < symbol_count {
        let base = symbol_table_offset + i * 18;
        let name = coff_symbol_name(bytes, base, string_table_offset, string_table_size)?;
        let value = read_u32(bytes, base + 8)?;
        let section_number = read_u16(bytes, base + 12)? as i16;
        let storage_class = bytes.get(base + 16).copied().unwrap_or(0);
        let aux_count = bytes.get(base + 17).copied().unwrap_or(0) as usize;

        if !name.is_empty() && section_number > 0 && storage_class != 103 {
            out.push(SymbolRecord { name, rva: value, source: SymbolSource::Coff });
        }

        i += 1 + aux_count;
    }

    Ok(())
}

fn coff_symbol_name(bytes: &[u8], base: usize, string_table_offset: usize, string_table_size: usize) -> Result<String, String> {
    if read_u32(bytes, base)? == 0 {
        let string_offset = read_u32(bytes, base + 4)? as usize;
        if string_offset < 4 || string_offset >= string_table_size {
            return Ok(String::new());
        }
        Ok(read_c_string(bytes, string_table_offset + string_offset))
    } else {
        let end = (base..base + 8).find(|&idx| bytes.get(idx).copied().unwrap_or(0) == 0).unwrap_or(base + 8);
        Ok(String::from_utf8_lossy(&bytes[base..end]).to_string())
    }
}

fn rva_to_offset(sections: &[Section], rva: u32) -> Option<usize> {
    for section in sections {
        let size = section.virtual_size.max(section.raw_size);
        if rva >= section.virtual_address && rva < section.virtual_address.saturating_add(size) {
            return Some(section.raw_pointer.saturating_add(rva - section.virtual_address) as usize);
        }
    }
    None
}

fn read_c_string(bytes: &[u8], offset: usize) -> String {
    if offset >= bytes.len() {
        return String::new();
    }
    let mut end = offset;
    while end < bytes.len() && bytes[end] != 0 {
        end += 1;
    }
    String::from_utf8_lossy(&bytes[offset..end]).to_string()
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = bytes.get(offset..offset + 2).ok_or_else(|| "unexpected end of file".to_string())?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes.get(offset..offset + 4).ok_or_else(|| "unexpected end of file".to_string())?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

pub fn minimal_empty_pe() -> Vec<u8> {
    let mut bytes = vec![0u8; 0x200];
    bytes[0] = b'M';
    bytes[1] = b'Z';
    bytes[0x3c] = 0x80;
    bytes[0x80] = b'P';
    bytes[0x81] = b'E';
    bytes[0x84] = 0x4c;
    bytes[0x85] = 0x01;
    bytes[0x94] = 0xe0;
    bytes[0x98] = 0x0b;
    bytes[0x99] = 0x01;
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_file_is_rejected() {
        assert!(parse_symbols(b"not pe").is_err());
    }

    #[test]
    fn minimal_pe_parses() {
        assert!(parse_symbols(&minimal_empty_pe()).unwrap().is_empty());
        assert!(parse_export_symbols(&minimal_empty_pe()).unwrap().is_empty());
        assert!(parse_coff_symbols_from_image(&minimal_empty_pe()).unwrap().is_empty());
    }
}

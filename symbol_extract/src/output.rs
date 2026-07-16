use crate::pe::SymbolRecord;
use std::io::Write;

pub fn write_text(symbols: &[SymbolRecord], mut writer: impl Write) -> Result<(), String> {
    for symbol in symbols {
        writeln!(writer, "{} 0x{:08X} {}", symbol.name, symbol.rva, symbol.source.as_str())
            .map_err(|err| format!("failed to write text payload: {err}"))?;
    }
    Ok(())
}

pub fn write_csv(symbols: &[SymbolRecord], mut writer: impl Write) -> Result<(), String> {
    writeln!(writer, "name,rva,source").map_err(|err| format!("failed to write csv payload: {err}"))?;
    for symbol in symbols {
        writeln!(writer, "\"{}\",0x{:08X},{}", escape_csv(&symbol.name), symbol.rva, symbol.source.as_str())
            .map_err(|err| format!("failed to write csv payload: {err}"))?;
    }
    Ok(())
}

pub fn write_json(symbols: &[SymbolRecord], mut writer: impl Write) -> Result<(), String> {
    writeln!(writer, "{{").map_err(|err| format!("failed to write json payload: {err}"))?;
    writeln!(writer, "  \"schema\": \"northstar.symbol_extract.symbols.v1\",").map_err(|err| format!("failed to write json payload: {err}"))?;
    writeln!(writer, "  \"symbol_count\": {},", symbols.len()).map_err(|err| format!("failed to write json payload: {err}"))?;
    writeln!(writer, "  \"symbols\": [").map_err(|err| format!("failed to write json payload: {err}"))?;
    for (idx, symbol) in symbols.iter().enumerate() {
        let comma = if idx + 1 == symbols.len() { "" } else { "," };
        writeln!(
            writer,
            "    {{ \"name\": \"{}\", \"rva\": \"0x{:08X}\", \"source\": \"{}\" }}{}",
            escape_json(&symbol.name),
            symbol.rva,
            symbol.source.as_str(),
            comma
        ).map_err(|err| format!("failed to write json payload: {err}"))?;
    }
    writeln!(writer, "  ]").map_err(|err| format!("failed to write json payload: {err}"))?;
    writeln!(writer, "}}").map_err(|err| format!("failed to write json payload: {err}"))?;
    Ok(())
}

fn escape_csv(value: &str) -> String {
    value.replace('"', "\"\"")
}

fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if other.is_control() => out.push_str(&format!("\\u{:04X}", other as u32)),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::SymbolSource;

    #[test]
    fn text_payload_has_no_status_tags() {
        let symbols = [SymbolRecord { name: "CreateToolhelp32Snapshot".to_string(), rva: 0x1230, source: SymbolSource::Export }];
        let mut bytes = Vec::new();
        write_text(&symbols, &mut bytes).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text, "CreateToolhelp32Snapshot 0x00001230 export\n");
        assert!(!text.contains("[INFO]"));
        assert!(!text.contains("[OK]"));
        assert!(!text.contains("\x1b["));
    }
}

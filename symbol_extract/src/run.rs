use crate::{
    cli::{Config, OutputFormat},
    output,
    pe::SymbolRecord,
    providers::{CoffProvider, PeExportProvider, SymbolProvider},
};
use std::{fs, io::Write, path::PathBuf};

pub fn extract(cfg: Config) -> Result<(), String> {
    let mut symbols = Vec::new();

    for input in &cfg.inputs {
        if !input.exists() {
            return Err(format!("input file does not exist: {}", input.display()));
        }

        let bytes = fs::read(input).map_err(|err| format!("failed to read {}: {err}", input.display()))?;
        let providers: [&dyn SymbolProvider; 2] = [&PeExportProvider::new(&bytes), &CoffProvider::new(&bytes)];

        for provider in providers {
            let mut provider_symbols = provider.enumerate().map_err(|err| format!("{}: {err}", input.display()))?;
            symbols.append(&mut provider_symbols);
        }
    }

    normalize_symbols(&mut symbols, &cfg);
    write_payload(&symbols, cfg.output.as_ref(), cfg.format)
}

fn normalize_symbols(symbols: &mut Vec<SymbolRecord>, cfg: &Config) {
    symbols.retain(|symbol| !cfg.exclusions.iter().any(|needle| symbol.name.contains(needle)));
    symbols.sort_by(|a, b| a.name.cmp(&b.name).then(a.rva.cmp(&b.rva)).then(a.source.as_str().cmp(b.source.as_str())));
    symbols.dedup();

    if let Some(max_count) = cfg.max_count {
        symbols.truncate(max_count);
    }
}

fn write_payload(symbols: &[SymbolRecord], output_path: Option<&PathBuf>, format: OutputFormat) -> Result<(), String> {
    match output_path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent).map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
                }
            }
            let mut file = fs::File::create(path).map_err(|err| format!("failed to create {}: {err}", path.display()))?;
            write_payload_to(symbols, &mut file, format)
        }
        None => {
            let stdout = std::io::stdout();
            let mut lock = stdout.lock();
            write_payload_to(symbols, &mut lock, format)?;
            lock.flush().map_err(|err| format!("failed to flush stdout: {err}"))
        }
    }
}

fn write_payload_to(symbols: &[SymbolRecord], writer: impl Write, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Text => output::write_text(symbols, writer),
        OutputFormat::Json => output::write_json(symbols, writer),
        OutputFormat::Csv => output::write_csv(symbols, writer),
    }
}

use super::SymbolProvider;
use crate::pe::{parse_export_symbols, SymbolRecord};

pub struct PeExportProvider<'a> {
    bytes: &'a [u8],
}

impl<'a> PeExportProvider<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl SymbolProvider for PeExportProvider<'_> {
    fn enumerate(&self) -> Result<Vec<SymbolRecord>, String> {
        parse_export_symbols(self.bytes)
    }
}

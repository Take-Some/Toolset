use super::SymbolProvider;
use crate::pe::{parse_coff_symbols_from_image, SymbolRecord};

pub struct CoffProvider<'a> {
    bytes: &'a [u8],
}

impl<'a> CoffProvider<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl SymbolProvider for CoffProvider<'_> {
    fn enumerate(&self) -> Result<Vec<SymbolRecord>, String> {
        parse_coff_symbols_from_image(self.bytes)
    }
}

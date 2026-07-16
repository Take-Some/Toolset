mod coff;
mod pe_export;

pub use coff::CoffProvider;
pub use pe_export::PeExportProvider;

use crate::pe::SymbolRecord;

pub trait SymbolProvider {
    fn enumerate(&self) -> Result<Vec<SymbolRecord>, String>;
}

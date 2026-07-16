pub mod properties;
pub mod xmlmeta;

pub use properties::{compile_ytyp_xml_to_properties, decode_ytyp_xml, inspect_ytyp_json};
pub use xmlmeta::{
    dependencies, entry_names, manifest_json_for_metadata, metadata_projection_json, summary_json,
    validate_metadata_xml,
};

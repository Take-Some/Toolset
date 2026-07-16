use std::path::PathBuf;

use northstar_neftd_packer::{
    font::{self, ImportOptions},
    nef8,
};

fn test_data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../toolbelt/first_party/northstar/neftd_packer/testData")
}

#[test]
fn packs_validates_and_extracts_fonts_from_toolbelt_test_data() {
    let dir = test_data_dir();
    assert!(
        dir.is_dir(),
        "missing testData directory: {}",
        dir.display()
    );

    let opts = ImportOptions {
        entry: None,
        family: Some("TT Lakes Neue Trial".to_owned()),
        style: None,
        weight: None,
    };
    let dict = font::import_sources(&[dir.clone()], &opts).expect("import test fonts");
    assert!(dict.entries.len() >= 3, "expected woff/ttf/otf fixtures");
    assert!(dict.entries.iter().any(|e| e.kind.label() == "woff"));
    assert!(dict.entries.iter().any(|e| e.kind.label() == "ttf"));
    assert!(dict.entries.iter().any(|e| e.kind.label() == "otf"));

    let bytes = nef8::pack_neftd(&dict, "fonts/test.neftd", true).expect("pack neftd");
    let (_header, entries) = nef8::parse_neftd(&bytes, "fonts/test.neftd").expect("parse neftd");
    assert_eq!(entries.len(), dict.entries.len());
    assert!(entries
        .iter()
        .all(|e| e.selector.starts_with("fonts/test.neftd@")));

    let first = &entries[0].name;
    let (_name, payload) =
        nef8::extract_entry(&bytes, "fonts/test.neftd", first).expect("extract first font");
    assert!(!payload.is_empty());
    assert!(
        font::FontKind::from_bytes(&payload).is_some(),
        "extracted payload must still be a font"
    );

    let inspect = nef8::inspect_json(&bytes, "fonts/test.neftd").expect("inspect json");
    assert_eq!(inspect["ok"], true);
    assert_eq!(inspect["content_kind"], "font_dictionary");
}

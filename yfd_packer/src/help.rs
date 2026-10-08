use std::io::{self, IsTerminal};

pub fn print_help() {
    println!(
        r#"northstar-yfd-packer

Purpose:
  Create, pack, inspect, list, validate and extract .yfd font dictionaries.
  .yfd = North Star Font Dictionary. It is a NEF8 ListFile asset.
  YFT remains free for a future fragment-like format.

Commands:
  create   --input font.ttf --output fonts/ui.yfd [--entry regular]
  pack     --input fonts/source_dir --output fonts/ui.yfd
  inspect  --input fonts/ui.yfd
  list     --input fonts/ui.yfd
  validate --input fonts/ui.yfd
  extract  --input fonts/ui.yfd --entry regular --out-dir out --overwrite

Source formats:
  TTF, OTF, WOFF, WOFF2, TTC.

Notes:
  This tool stores validated font source bytes and metadata in a native font dictionary.
  It does not shape text, rasterize glyphs, or depend on a renderer/text backend.
"#
    );
    println!();
    println!("Common commands:");
    println!("  pack/build/compile/create/import     Build a runtime asset where supported.");
    println!(
        "  inspect | validate | doctor          Inspect or validate an existing runtime asset."
    );
    println!("  accepted-inputs                      Print accepted input/output contract.");
    println!("  version                              Print tool version.");
    println!();
    println!("Accepted input files: *.ttf, *.otf, *.ttc, *.woff, *.woff2 font sources; *.yfd for inspect/list/validate/extract");
    println!(
        "Produced output files: *.yfd runtime NEF8 font dictionary; extracted *.fontbin payloads"
    );
    println!(
        "Output modes: default production output; add --debug or --verbose for debug diagnostics."
    );
}
pub fn wait_for_enter() {
    if io::stdin().is_terminal() {
        println!("Press Enter to close...");
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}

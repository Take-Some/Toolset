use std::io;

pub fn print_help() {
    println!("North Star YTYP Packer / Y-Type Properties tool");
    println!();
    println!("Canonical authoring source: *.ytyp.xml with Y-Type Properties XML.");
    println!("Runtime asset: *.ytyp is the compiled properties document for one model/asset/type.");
    println!("YTYP = Y-Type Properties. It is not a dictionary; files ending with 'd' are dictionaries (.ydd/.ytd/.neftd).");
    println!();
    println!("Usage:");
    println!("  northstar-ytyp-packer compile --root <repo-or-asset-root> --input world/src/fps/tree.ytyp.xml --output world/fps/tree.ytyp");
    println!("  northstar-ytyp-packer compile --root <repo-or-asset-root> --all [--check]");
    println!("  northstar-ytyp-packer inspect --input world/fps/tree.ytyp");
    println!(
        "  northstar-ytyp-packer dump-xml --input world/fps/tree.ytyp [--output tree.ytyp.xml]"
    );
    println!("  northstar-ytyp-packer validate --root <repo-or-asset-root> --all");
    println!("  northstar-ytyp-packer manifest --input world/fps/tree.ytyp");
    println!("  northstar-ytyp-packer dump-metadata --input world/fps/tree.ytyp");
    println!("  northstar-ytyp-packer dump-dependencies --input world/fps/tree.ytyp");
    println!();
    println!("Common commands:");
    println!(
        "  pack/build/compile/create/import     Build runtime properties asset where supported."
    );
    println!(
        "  inspect | validate | doctor          Inspect or validate an existing properties asset."
    );
    println!("  accepted-inputs                      Print accepted input/output contract.");
    println!("  version                              Print tool version.");
    println!();
    println!("Accepted input files: *.ytyp.xml source properties; *.ytyp compiled properties assets for inspect/validate/dump");
    println!("Produced output files: *.ytyp properties assets; XML dumps; JSON manifest/metadata/dependency projections");
    println!(
        "Output modes: default production output; add --debug or --verbose for debug diagnostics."
    );
}
pub fn wait_for_enter() {
    println!();
    println!("This tool works through arguments. Press Enter to close...");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

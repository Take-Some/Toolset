use std::io;

pub fn print_help() {
    println!("North Star YTD Packer / NETD texture dictionary tool");
    println!();
    println!("Runtime asset: *.ytd is NEF8/ListFile content_kind=1 with a binary NETD texture dictionary body.");
    println!("There is no XML source-of-truth for .ytd textures. Pack from texture files, inspect NETD, extract DDS.");
    println!();
    println!("Usage:");
    println!("  northstar-ytd-packer pack --texture logo=logo.png --texture icon=icon.png --output ui_icons.ytd");
    println!("  northstar-ytd-packer pack --input-dir textures/ui --output ui_icons.ytd [--linear] [--linear-entry NAME] [--no-mips] [--raw-data]");
    println!("  northstar-ytd-packer inspect --input ui_icons.ytd");
    println!("  northstar-ytd-packer validate --input ui_icons.ytd");
    println!("  northstar-ytd-packer extract --input ui_icons.ytd --output extracted_dds [--entry texture_name]");
    println!("  northstar-ytd-packer dump-netd --input ui_icons.ytd --output payload.netd");
    println!();
    println!("Common commands:");
    println!("  pack/build/compile/create/import     Build a runtime asset where supported.");
    println!(
        "  inspect | validate | doctor          Inspect or validate an existing runtime asset."
    );
    println!("  accepted-inputs                      Print accepted input/output contract.");
    println!("  version                              Print tool version.");
    println!();
    println!("Accepted input files: *.png, *.bmp, *.jpg, *.jpeg, *.dds, *.tga texture sources; *.ytd for inspect/validate/extract/dump-netd");
    println!("Produced output files: *.ytd runtime NEF8 texture dictionary; extracted *.dds files; *.netd body dumps");
    println!(
        "Output modes: default production output; add --debug or --verbose for debug diagnostics."
    );
}
pub fn wait_for_enter() {
    println!("\nThis tool works through arguments. Press Enter to close...");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

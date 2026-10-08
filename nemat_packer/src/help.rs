use std::io;

pub fn print_help() {
    println!("LEGACY FORMAT: NEMAT is read/migration compatibility only. New materials use YMAT or YDD/YDR built-in MaterialResource.");
    println!("North Star NEMAT Legacy Reader / Migration Compatibility Tool");
    println!();
    println!("Supported legacy inputs: *.nemat and *.nemat.xml.");
    println!("New authored materials must use YMAT or YDD/YDR built-in MaterialResource.");
    println!("Use Tools/toolsSrc/ymat_packer/migrate_nemat_xml_to_ymat.py for one-time authored XML migration.");
    println!();
    println!("Usage:");
    println!("  northstar-nemat-packer validate --input materials/world/garage.nemat");
    println!("  northstar-nemat-packer inspect --input materials/world/garage.nemat");
    println!("  northstar-nemat-packer dump-xml --input materials/world/garage.nemat --output garage.decoded.nemat.xml");
    println!("  northstar-nemat-packer manifest --input materials/world/garage.nemat");
    println!("  northstar-nemat-packer graph --input materials/world/garage.nemat");
    println!();
    println!("Common commands:");
    println!("  pack/build/compile/create/import     Build a runtime asset where supported.");
    println!(
        "  inspect | validate | doctor          Inspect or validate an existing runtime asset."
    );
    println!("  accepted-inputs                      Print accepted input/output contract.");
    println!("  version                              Print tool version.");
    println!();
    println!("Accepted input files: *.nemat.xml XMLtype material libraries and *.nemat NEF8 material libraries for inspect/validate/dump");
    println!("Produced output files: *.nemat.xml/XML dumps; JSON manifest/graph projections");
    println!(
        "Output modes: default production output; add --debug or --verbose for debug diagnostics."
    );
}
pub fn wait_for_enter() {
    println!("\nThis tool works through arguments. Press Enter to close...");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

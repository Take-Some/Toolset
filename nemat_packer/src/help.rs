use std::io;

pub fn print_help() {
    println!("North Star NEMAT Packer / XMLtype material library tool");
    println!();
    println!("Authored source: *.nemat.xml with <NematMaterialLibrary schema=\"newengine.nemat.xmltype.v1\">.");
    println!("Runtime asset: *.nemat is NEF8/ListFile content_kind=4 with the XMLtype material_library body.");
    println!("Material entries are addressed as file.nemat@entry and texture slots must reference .ytd@entry selectors.");
    println!("JSON output from this tool is diagnostic projection only, not an authored material format.");
    println!();
    println!("Usage:");
    println!("  northstar-nemat-packer create-draft --material garage_door --texture base_color=textures/garage.ytd@door_bc --output garage.nemat.xml");
    println!("  northstar-nemat-packer validate --input garage.nemat.xml");
    println!("  northstar-nemat-packer pack --input garage.nemat.xml --output materials/world/garage.nemat");
    println!("  northstar-nemat-packer inspect --input materials/world/garage.nemat");
    println!("  northstar-nemat-packer dump-xml --input materials/world/garage.nemat --output garage.decoded.nemat.xml");
    println!("  northstar-nemat-packer manifest --input materials/world/garage.nemat");
    println!("  northstar-nemat-packer graph --input materials/world/garage.nemat");
    println!();
    println!("Draft options:");
    println!("  --material NAME              Material entry name. Default: material");
    println!("  --shader ID                  Shader/material contract id. Default: pbr.default");
    println!("  --blend opaque|masked|alpha  Surface blend mode. Default: opaque");
    println!("  --two-sided                  Mark surface two-sided");
    println!("  --alpha-cutoff VALUE         Alpha cutoff for masked surfaces");
    println!("  --texture slot=path.ytd@entry  Add texture binding. Can be repeated");
    println!("  --param name:type=value      Add typed param. Types: float,float2,float3,float4,int,bool,color,enum,texture_ref");
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
    println!("Produced output files: *.nemat runtime NEF8 material library; *.nemat.xml/XML dumps; JSON manifest/graph projections");
    println!(
        "Output modes: default production output; add --debug or --verbose for debug diagnostics."
    );
}
pub fn wait_for_enter() {
    println!("\nThis tool works through arguments. Press Enter to close...");
    let mut line = String::new();
    let _ = io::stdin().read_line(&mut line);
}

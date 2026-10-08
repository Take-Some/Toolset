use std::io::{self, IsTerminal};

pub fn print_help() {
    println!(
        r#"northstar-ydd-packer

Purpose:
  Build and inspect .ydd as a resident NEF8 ListFile drawable dictionary.
  A .ydd stores a set of resident model entries and is addressed as file.ydd@model_name.

Commands:
  pack      --input model.obj --input model.glb --output file.ydd [--material file.ymat@mat]
  inspect   --input file.ydd
  list      --input file.ydd
  validate  --input file.ydd
  extract-material    --input file.ydd [--entry model] [--mesh mesh] --output material.ymat
  externalize-material --input file.ydd [--entry model] [--mesh mesh] --material materials/file.ymat@entry [--material-file local.ymat] --output patched.ydd
  embed-material      --input file.ydd [--entry model] [--mesh mesh] --material materials/file.ymat@entry [--material-file local.ymat] --output patched.ydd
  set-material-ref    --input file.ydd [--entry model] [--mesh mesh] --material materials/file.ymat@entry --output patched.ydd
  doctor   [--input file.ydd] [--fix]
  dump-body --input file.ydd --output file.yddbody

Source formats:
  OBJ   native importer, triangulates polygon faces by fan.
  glTF  native .gltf accessor/buffer reader for POSITION/NORMAL/TEXCOORD_0/indices.
  GLB   native .glb v2 reader for JSON and BIN chunks.
  FBX   native ASCII geometry importer for Mesh Vertices/PolygonVertexIndex.

Options:
  --input, -i           Source model or .ydd depending on command. May repeat for pack.
  --output, -o          Output .ydd or dump path.
  --entry, --name       Model entry selector; for pack, overrides a single source entry name.
  --mesh                 Mesh selector for material authoring commands.
  --material             External logical .ymat@entry selector.
  --material-file        Physical .ymat path when it differs from the logical --material path.
  --scale               Position scale applied on import. Default: 1.0.
  --flip-v              Flip imported V coordinate.
  --no-triangulate      Reject non-triangle OBJ/glTF topology instead of triangulating/trimming.

Examples:
  northstar-ydd-packer pack -i assets/src/body.obj -i assets/src/head.glb -o assets/models/ped.ydd
  northstar-ydd-packer list -i assets/models/ped.ydd
  northstar-ydd-packer inspect -i assets/models/ped.ydd
  northstar-ydd-packer externalize-material -i assets/models/ped.ydd --entry ped --mesh body --material materials/ped.ymat@body -o assets/models/ped.external.ydd
  northstar-ydd-packer embed-material -i assets/models/ped.external.ydd --entry ped --mesh body --material materials/ped.ymat@body -o assets/models/ped.embedded.ydd
  northstar-ydd-packer doctor --input assets/models/ped.ydd --fix
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
    println!("Accepted input files: *.obj, *.gltf, *.glb, ASCII *.fbx model sources; *.ydd for inspect/list/validate/dump-body");
    println!("Produced output files: *.ydd runtime NEF8 drawable dictionary; *.yddbody body dumps");
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

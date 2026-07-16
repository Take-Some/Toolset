use std::io::{self, IsTerminal};

pub fn print_help() {
    println!(r#"northstar-nepak-manager

Purpose:
  Manage clean .nepak VFS packages: pack, extract, inspect, list,
  verify, emit manifests, compare packages and dry-run mount metadata.

Clean format rule:
  .nepak is a VFS package only. It stores bytes, path metadata,
  package/profile metadata, content_kind hints and storage accounting.
  It does not parse .ytd/.ydd/.ytyp/.nemat semantics.

Commands:
  pack       --input <file-or-directory> --output package.nepak [--no-compress]
  extract    --input package.nepak --output <directory> [--path package/path] [--overwrite]
  inspect    --input package.nepak
  manifest   --input package.nepak
  list       --input package.nepak
  verify     --input package.nepak
  mount-test --input package.nepak
  diff       --input old.nepak --compare new.nepak
  doctor
  accepted-inputs
  version

Options:
  --input, -i       Input file, input directory, or package depending on command.
  --output, -o      Output .nepak or extraction directory.
  --compare         Second package for diff.
  --path, --entry   Extract only one package path.
  --overwrite       Allow extract to overwrite existing files.
  --no-compress     Store payloads raw instead of deflate-compressed.
  --debug           Status diagnostics only; payload commands remain clean stdout.

Examples:
  northstar-nepak-manager pack -i assets/runtime -o builds/runtime.nepak
  northstar-nepak-manager inspect -i builds/runtime.nepak > inspect.json
  northstar-nepak-manager manifest -i builds/runtime.nepak > manifest.json
  northstar-nepak-manager list -i builds/runtime.nepak > entries.txt
  northstar-nepak-manager verify -i builds/runtime.nepak
  northstar-nepak-manager mount-test -i builds/runtime.nepak > mount.json
  northstar-nepak-manager diff -i old.nepak --compare new.nepak > diff.json
  northstar-nepak-manager extract -i builds/runtime.nepak -o .takesome/extract/nepak --overwrite
"#);
    println!();
    println!("Accepted input files: directory trees and opaque loose files for pack; *.nepak for inspect/manifest/list/verify/extract/mount-test/diff");
    println!("Produced output files: *.nepak VFS container; extracted directory trees; clean JSON/list payloads");
    println!("Output modes: status output for pack/verify/extract/doctor; clean raw payload stdout for inspect/manifest/list/mount-test/diff.");
}

pub fn wait_for_enter() {
    if io::stdin().is_terminal() {
        println!("Press Enter to close...");
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}

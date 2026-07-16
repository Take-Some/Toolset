pub fn print_help() {
    println!(
        "North Star YMAP Packer\n\n\
         Commands:\n\
           compile --input <file.ymap.xml|file.ymap.json> --output <file.ymap> [--logical-path <maps/foo.ymap>]\n\
           compile --root <repo-or-assets-root> --all [--dry-run|--check]\n\
           dump --input <file.ymap> [--output <file.ymap.xml>]\n\
           inspect --input <file.ymap>\n\
           validate --input <file.ymap>\n\
           validate --root <repo-or-assets-root> --all\n\
           doctor\n\
           accepted-inputs\n\n\
         Policy:\n\
           Runtime reads .ymap only through AssetManager / NEF8 decode_v1.\n\
           This tool is authoring-side: readable XML/JSON source in, binary NEF8 .ymap out."
    );
}

pub fn wait_for_enter() {
    if std::env::var("NORTHSTAR_TOOL_NO_WAIT").is_ok() {
        return;
    }
    if std::env::args().any(|arg| arg == "--no-wait") {
        return;
    }
}

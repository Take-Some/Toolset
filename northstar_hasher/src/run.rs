use crate::{cli::{output_line, strip_extension, Config}, hash::joaat_hash};
use std::{fs::File, io::{BufRead, BufReader}};

pub fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(&config.filename)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let raw = line?;
        let trimmed = raw.trim();
        if trimmed.is_empty() { continue; }

        let name = if config.strip_ext { strip_extension(trimmed).to_string() } else { trimmed.to_string() };
        let hash = joaat_hash(&name, config.literal);
        if config.target_hash.is_some_and(|target| target != hash) { continue; }
        println!("{}", output_line(&name, config.literal));
    }

    Ok(())
}

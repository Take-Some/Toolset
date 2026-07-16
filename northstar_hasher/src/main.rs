#![windows_subsystem = "console"]

mod cli;
mod console;
mod hash;
mod run;

use crate::cli::{accepted_inputs_text, doctor_text, parse_args, usage, Command, VERSION};
use std::{env, process};

fn main() {
    match parse_args(env::args().skip(1)) {
        Ok(Command::Help) => { println!("{}", usage()); console::pause_if_interactive(); }
        Ok(Command::Version) => println!("northstar-hasher {}", VERSION),
        Ok(Command::AcceptedInputs) => println!("{}", accepted_inputs_text()),
        Ok(Command::Doctor) => println!("{}", doctor_text()),
        Ok(Command::Hash(config)) => {
            if let Err(err) = run::run(config) {
                eprintln!("error: {}", err);
                console::pause_if_interactive();
                process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("{}", usage());
            console::pause_if_interactive();
            process::exit(2);
        }
    }
}

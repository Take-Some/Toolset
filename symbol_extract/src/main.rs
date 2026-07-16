#![windows_subsystem = "console"]

mod cli;
mod console;
mod diagnostics;
mod output;
mod pe;
mod providers;
mod run;

use crate::cli::{parse_args, Command};
use std::{env, process};

fn main() {
    match parse_args(env::args().skip(1)) {
        Ok(Command::Help) => { println!("{}", cli::usage()); console::pause_if_interactive(); },
        Ok(Command::Version) => diagnostics::print_version(),
        Ok(Command::AcceptedInputs) => diagnostics::print_accepted_inputs(),
        Ok(Command::Doctor) => process::exit(diagnostics::doctor()),
        Ok(Command::Extract(config)) => {
            if let Err(err) = run::extract(config) {
                diagnostics::print_error(err);
                console::pause_if_interactive();
                process::exit(1);
            }
        }
        Err(err) => {
            diagnostics::print_error(err);
            eprintln!("{}", cli::usage());
            console::pause_if_interactive();
            process::exit(2);
        }
    }
}

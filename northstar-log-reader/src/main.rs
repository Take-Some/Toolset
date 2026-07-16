#![windows_subsystem = "console"]

mod app;
mod cli;
mod commands;
mod event;
mod http;
mod input;
mod output;
mod ui;

use northstar_cli::ansi;
use std::{env, process};

fn main() {
    if let Err(err) = cli::dispatch(env::args().skip(1).collect()) {
        ansi::error(err);
        process::exit(1);
    }
}

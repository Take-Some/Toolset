use std::io::{self, IsTerminal, Write};

pub fn pause_if_interactive() {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        print!("\nPress Enter to close...");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}

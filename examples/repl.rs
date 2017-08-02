//! This example shows a simple read-evaluate-print-loop (REPL).

extern crate rlua;
extern crate rustyline;

use rlua::{Lua, MultiValue, Error};
use rustyline::Editor;

fn main() {
    let lua = Lua::new();
    let mut editor = Editor::<()>::new();

    loop {
        let mut prompt = "> ";
        let mut line = String::new();

        loop {
            match editor.readline(prompt) {
                Ok(input) => line.push_str(&input),
                Err(_) => return,
            }

            match lua.eval::<MultiValue>(&line, None) {
                Ok(values) => {
                    editor.add_history_entry(&line);
                    println!(
                        "{}",
                        values
                            .iter()
                            .map(|value| format!("{:?}", value))
                            .collect::<Vec<_>>()
                            .join("\t")
                    );
                    break;
                }
                Err(Error::IncompleteStatement(_)) => {
                    // continue reading input and append it to `line`
                    prompt = ">> ";
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    break;
                }
            }
        }
    }
}

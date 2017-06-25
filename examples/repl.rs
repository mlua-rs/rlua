//! This example shows a simple read-evaluate-print-loop (REPL).

extern crate rlua;

use rlua::*;
use std::io::prelude::*;
use std::io::{stdin, stdout, stderr, BufReader};

fn main() {
    let lua = Lua::new();
    let mut stdout = stdout();
    let mut stdin = BufReader::new(stdin());

    loop {
        write!(stdout, "> ").unwrap();
        stdout.flush().unwrap();

        let mut line = String::new();

        loop {
            stdin.read_line(&mut line).unwrap();

            match lua.eval::<LuaMultiValue>(&line) {
                Ok(values) => {
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
                Err(LuaError::IncompleteStatement(_)) => {
                    // continue reading input and append it to `line`
                    write!(stdout, ">> ").unwrap();
                    stdout.flush().unwrap();
                }
                Err(e) => {
                    writeln!(stderr(), "error: {}", e).unwrap();
                    break;
                }
            }
        }
    }
}

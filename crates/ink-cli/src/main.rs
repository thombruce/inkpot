//! `ink render --view=manuscript|outline|edit|codex <file.ink>`

use ink_core::{parse, render, View};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut view = View::Manuscript;
    let mut path: Option<String> = None;

    let mut rest = args.iter();
    // First arg is the subcommand ("render"); accept it or skip.
    let mut it = rest.by_ref().peekable();
    if it.peek().map(|s| s.as_str()) == Some("render") {
        it.next();
    }
    for arg in it {
        if let Some(v) = arg.strip_prefix("--view=") {
            view = match v {
                "manuscript" => View::Manuscript,
                "outline" => View::Outline,
                "edit" => View::Edit,
                "codex" => View::Codex,
                other => return err(format!("unknown view '{other}'")),
            };
        } else if arg.starts_with('-') {
            return err(format!("unknown flag '{arg}'"));
        } else {
            path = Some(arg.clone());
        }
    }

    let Some(path) = path else {
        return err("usage: ink render --view=manuscript|outline|edit|codex <file.ink>".into());
    };
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => return err(format!("{path}: {e}")),
    };

    print!("{}", render(&parse(&src), view));
    ExitCode::SUCCESS
}

fn err(msg: String) -> ExitCode {
    eprintln!("ink: {msg}");
    ExitCode::FAILURE
}

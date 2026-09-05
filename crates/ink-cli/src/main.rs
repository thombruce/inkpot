//! `ink render --view=manuscript|outline|edit|codex <file.ink>`
//! `ink export --out=<file.pdf> [--trim=WxH_mm] <file.ink>` (Shunn PDF)

use ink_core::shunn::{render_shunn_pdf, render_shunn_pdf_sized};
use ink_core::{build_shunn, parse, render, View};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("export") {
        return export(&args[1..]);
    }

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

/// `ink export --out=<file.pdf> [--trim=WxH] <file.ink>` — render a single
/// document to a Shunn manuscript PDF. `--trim` gives a custom page size in mm
/// (e.g. `152.4x228.6` for KDP 6×9); default is US Letter.
fn export(args: &[String]) -> ExitCode {
    let mut out: Option<String> = None;
    let mut trim: Option<(f64, f64)> = None;
    let mut path: Option<String> = None;
    for arg in args {
        if let Some(o) = arg.strip_prefix("--out=") {
            out = Some(o.to_string());
        } else if let Some(t) = arg.strip_prefix("--trim=") {
            match t.split_once(['x', 'X']).and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))) {
                Some(wh) => trim = Some(wh),
                None => return err(format!("bad --trim '{t}', want WxH in mm e.g. 152.4x228.6")),
            }
        } else if arg.starts_with('-') {
            return err(format!("unknown flag '{arg}'"));
        } else {
            path = Some(arg.clone());
        }
    }
    let (Some(path), Some(out)) = (path, out) else {
        return err("usage: ink export --out=<file.pdf> [--trim=WxH] <file.ink>".into());
    };
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => return err(format!("{path}: {e}")),
    };
    let m = build_shunn(&parse(&src));
    let bytes = match trim {
        Some((w, h)) => render_shunn_pdf_sized(&m, w, h),
        None => render_shunn_pdf(&m),
    };
    let bytes = match bytes {
        Ok(b) => b,
        Err(e) => return err(e),
    };
    if let Err(e) = std::fs::write(&out, bytes) {
        return err(format!("{out}: {e}"));
    }
    ExitCode::SUCCESS
}

fn err(msg: String) -> ExitCode {
    eprintln!("ink: {msg}");
    ExitCode::FAILURE
}

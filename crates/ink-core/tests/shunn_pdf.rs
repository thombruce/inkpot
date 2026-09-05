// Runs only with `--features pdf`. Smoke-tests the genpdf emitter end to end:
// build the model from source, render bytes, and confirm it's a real PDF. The
// structure/rounding logic is covered by the always-on tests; this guards the
// emitter wiring (fonts embed, genpdf renders, both paper sizes work).
#![cfg(feature = "pdf")]

use ink_core::shunn::{render_shunn_pdf, render_shunn_pdf_sized};
use ink_core::{build_shunn, build_shunn_book, parse};

const DOC: &str = "title: The Book\nauthor: Thom Bruce\ncontact:\n  1 Example St\n  t@example.com\n\n\
                   # Chapter One\n\nProse one.\n\n~ Scene\n\nProse two.\n\n# Chapter Two\n\nMore.\n";

#[test]
fn emits_a_valid_pdf_on_letter_and_custom_trim() {
    let m = build_shunn(&parse(DOC));

    let letter = render_shunn_pdf(&m).expect("render letter");
    assert!(letter.starts_with(b"%PDF-"), "not a PDF");
    assert!(letter.len() > 2_000, "suspiciously small: {} bytes", letter.len());

    // KDP 6x9" custom trim goes through the sized entry point.
    let sized = render_shunn_pdf_sized(&m, 152.4, 228.6).expect("render 6x9");
    assert!(sized.starts_with(b"%PDF-"), "custom-trim not a PDF");
}

#[test]
fn emits_a_valid_book_pdf_from_multiple_files() {
    let marker = parse("title: The Work\nauthor: Jane Roe\n");
    let ch1 = parse("# One\n\nAlpha beta gamma delta.\n");
    let ch2 = parse("# Two\n\nEpsilon zeta eta theta.\n");
    let book = build_shunn_book(&marker, &[&ch1, &ch2]);
    let pdf = render_shunn_pdf(&book).expect("render book");
    assert!(pdf.starts_with(b"%PDF-"), "book not a PDF");
}

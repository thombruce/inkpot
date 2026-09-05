//! Shunn Proper Manuscript Format export (#23).
//!
//! Two layers, split so the pure part is always compiled and tested while the
//! PDF engine stays optional:
//!
//! - **The manuscript model** ([`ShunnManuscript`]) — a flat, route-independent
//!   projection of the document: title-page fields + a stream of [`ShunnBlock`]s
//!   (chapters, scene breaks, prose). This is the *seam*: the same model can feed
//!   a future EPUB or KDP-print emitter (#80). Built from the [`Node`](crate::Node)
//!   tree by `build_shunn` (in `render`, where inline resolution lives).
//! - **The PDF emitter** ([`render_shunn_pdf`]) — gated behind the `pdf` feature
//!   (pulls `genpdf`), so a default `cargo test` and the app's fast path stay
//!   pure. Fonts are embedded, so there is no runtime file dependency.

/// One block of a Shunn manuscript, in reading order.
#[derive(Debug, Clone, PartialEq)]
pub enum ShunnBlock {
    /// A chapter start — a new page with the (centered) title. Its presence makes
    /// the document a *novel*; a manuscript with none is a *short story* whose
    /// prose flows on from the title page.
    Chapter(String),
    /// A visible heading below chapter level — a centered subhead.
    Subhead(String),
    /// A scene break within a chapter — a centered `#`.
    SceneBreak,
    /// A body paragraph (prose, already resolved: markup/links/interpolation).
    Para(String),
}

/// The title-page fields + block stream for a Shunn manuscript.
#[derive(Debug, Clone, PartialEq)]
pub struct ShunnManuscript {
    pub title: String,
    /// Contact block, top-left on the first page (name/address/email/phone).
    pub contact: Vec<String>,
    /// Publish-under name, centered under the title.
    pub byline: String,
    /// Running-header surname (last word of the author's name).
    pub surname: String,
    /// Running-header keyword (a short form of the title).
    pub keyword: String,
    /// Whole-manuscript word count, already rounded per Shunn.
    pub words: usize,
    pub blocks: Vec<ShunnBlock>,
}

/// Round a word count the Shunn way: to the nearest 100 below ~10 000, and to
/// the nearest 1 000 at or above it (a cover page never shows a false-precise
/// count). 0 stays 0.
pub fn round_wordcount(n: usize) -> usize {
    let step = if n < 10_000 { 100 } else { 1_000 };
    ((n + step / 2) / step) * step
}

/// The running-header surname: the last whitespace-separated word of the author's
/// name, or empty if there is none.
pub fn surname(author: &str) -> String {
    author.split_whitespace().last().unwrap_or("").to_string()
}

/// The running-header keyword: the title's first *distinctive* word, uppercased
/// — a short handle the editor recognises. A leading article (`the`/`a`/`an`) is
/// skipped so "The Book" yields `BOOK`, not the useless `THE` (unless the title
/// is only that article). Empty title yields an empty keyword.
pub fn header_keyword(title: &str) -> String {
    let mut words = title.split_whitespace();
    let first = words.next().unwrap_or("");
    let is_article = matches!(first.to_ascii_lowercase().as_str(), "the" | "a" | "an");
    let word = if is_article { words.next().unwrap_or(first) } else { first };
    word.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordcount_rounds_per_shunn() {
        assert_eq!(round_wordcount(0), 0);
        assert_eq!(round_wordcount(49), 0); // nearest 100, rounds down
        assert_eq!(round_wordcount(50), 100); // .5 rounds up
        assert_eq!(round_wordcount(7_649), 7_600);
        assert_eq!(round_wordcount(7_650), 7_700);
        assert_eq!(round_wordcount(9_950), 10_000); // crosses into the 1000-step band
        // At/above 10k: nearest 1000.
        assert_eq!(round_wordcount(80_400), 80_000);
        assert_eq!(round_wordcount(80_500), 81_000);
    }

    #[test]
    fn surname_and_keyword() {
        assert_eq!(surname("Thom Bruce"), "Bruce");
        assert_eq!(surname("Ursula K. Le Guin"), "Guin");
        assert_eq!(surname(""), "");
        assert_eq!(header_keyword("The Example Novel"), "EXAMPLE"); // article skipped
        assert_eq!(header_keyword("An Ode"), "ODE");
        assert_eq!(header_keyword("The"), "THE"); // article-only title keeps it
        assert_eq!(header_keyword("Dune"), "DUNE");
        assert_eq!(header_keyword(""), "");
    }
}

// --- PDF emitter (optional, pulls genpdf) -----------------------------------

#[cfg(feature = "pdf")]
mod pdf {
    use super::{ShunnBlock, ShunnManuscript};
    use genpdf::elements::{Break, PageBreak, Paragraph};
    use genpdf::{fonts, Alignment, Document, Margins, SimplePageDecorator, Size};

    const IN: f64 = 25.4; // 1 inch in mm

    // Courier Prime (OFL), embedded so there is no runtime font dependency.
    const REGULAR: &[u8] = include_bytes!("../fonts/CourierPrime-Regular.ttf");
    const BOLD: &[u8] = include_bytes!("../fonts/CourierPrime-Bold.ttf");
    const ITALIC: &[u8] = include_bytes!("../fonts/CourierPrime-Italic.ttf");
    const BOLD_ITALIC: &[u8] = include_bytes!("../fonts/CourierPrime-BoldItalic.ttf");

    // Courier is 10 chars/inch at 12pt; a 0.5" first-line indent is 5 chars.
    // genpdf has no first-line-indent primitive, so we prepend spaces (safe in a
    // monospace face). ponytail: monospace-only hack; a real indent needs krilla.
    const INDENT: &str = "     ";

    fn font_family() -> fonts::FontFamily<fonts::FontData> {
        let load = |b: &[u8]| fonts::FontData::new(b.to_vec(), None).expect("embedded font parses");
        fonts::FontFamily {
            regular: load(REGULAR),
            bold: load(BOLD),
            italic: load(ITALIC),
            bold_italic: load(BOLD_ITALIC),
        }
    }

    fn centered(text: impl Into<String>) -> Paragraph {
        let mut p = Paragraph::new(text.into());
        p.set_alignment(Alignment::Center);
        p
    }

    /// Render a [`ShunnManuscript`] to PDF bytes at the given paper size (mm).
    /// Font parsing can't fail (fonts are embedded and tested); a genpdf layout
    /// error propagates as a message so a caller behind an IPC boundary can
    /// return it instead of unwinding.
    pub fn render(m: &ShunnManuscript, paper: Size) -> Result<Vec<u8>, String> {
        let mut doc = Document::new(font_family());
        doc.set_font_size(12);
        doc.set_line_spacing(2.0); // Shunn: double-spaced
        if !m.title.is_empty() {
            doc.set_title(&m.title);
        }
        doc.set_paper_size(paper);

        // Running header `Surname / KEYWORD / page#`, right-aligned, page 2 on.
        let surname = m.surname.clone();
        let keyword = m.keyword.clone();
        let mut deco = SimplePageDecorator::new();
        deco.set_margins(Margins::trbl(IN, IN, IN, IN));
        deco.set_header(move |page| {
            let mut p = Paragraph::new("");
            if page > 1 {
                p = Paragraph::new(format!("{surname} / {keyword} / {page}"));
                p.set_alignment(Alignment::Right);
            }
            p
        });
        doc.set_page_decorator(deco);

        // First page: contact block top-left, word count, then title/byline down
        // the page. Flow layout, so placement is approximate (see module notes).
        for line in &m.contact {
            doc.push(Paragraph::new(line));
        }
        if m.words > 0 {
            let mut wc = Paragraph::new(format!("about {} words", m.words));
            wc.set_alignment(Alignment::Right);
            doc.push(wc);
        }
        doc.push(Break::new(8.0));
        if !m.title.is_empty() {
            doc.push(centered(m.title.to_uppercase()));
        }
        if !m.byline.is_empty() {
            doc.push(centered(format!("by {}", m.byline)));
        }

        for block in &m.blocks {
            match block {
                ShunnBlock::Chapter(title) => {
                    doc.push(PageBreak::new());
                    doc.push(Break::new(6.0)); // chapters start partway down
                    doc.push(centered(title));
                    doc.push(Break::new(2.0));
                }
                ShunnBlock::Subhead(title) => {
                    doc.push(Break::new(1.0));
                    doc.push(centered(title));
                    doc.push(Break::new(1.0));
                }
                ShunnBlock::SceneBreak => {
                    doc.push(Break::new(1.0));
                    doc.push(centered("#"));
                    doc.push(Break::new(1.0));
                }
                ShunnBlock::Para(text) => {
                    doc.push(Paragraph::new(format!("{INDENT}{text}")));
                }
            }
        }
        doc.push(Break::new(1.0));
        doc.push(centered("THE END"));

        let mut out = Vec::new();
        doc.render(&mut out).map_err(|e| format!("genpdf render: {e}"))?;
        Ok(out)
    }
}

/// Render a [`ShunnManuscript`] to PDF bytes on US Letter (the Shunn submission
/// size). Custom trim sizes (e.g. KDP 6×9) go through [`render_shunn_pdf_sized`].
#[cfg(feature = "pdf")]
pub fn render_shunn_pdf(m: &ShunnManuscript) -> Result<Vec<u8>, String> {
    pdf::render(m, genpdf::Size::new(215.9, 279.4))
}

/// Render to PDF bytes at an arbitrary paper size in millimetres (width, height)
/// — the seam a future KDP-print template uses.
#[cfg(feature = "pdf")]
pub fn render_shunn_pdf_sized(
    m: &ShunnManuscript,
    width_mm: f64,
    height_mm: f64,
) -> Result<Vec<u8>, String> {
    pdf::render(m, genpdf::Size::new(width_mm, height_mm))
}

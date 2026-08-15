use ink_core::{parse, render, Block, Inline, View};

const SAMPLE: &str = include_str!("../../../examples/sample.ink");

#[test]
fn tree_shape_and_nesting() {
    let root = parse(SAMPLE);
    // Two top-level visible chapters.
    assert_eq!(root.children.len(), 2);
    let ch1 = &root.children[0];
    assert_eq!(ch1.title, "Chapter 1");
    assert!(ch1.visible);
    assert_eq!(ch1.level, 1);

    // Chapter 1 -> The Arrival (##).
    let arrival = &ch1.children[0];
    assert_eq!(arrival.title, "The Arrival");
    assert_eq!(arrival.level, 2);

    // The Arrival holds two invisible scenes as peers.
    let scenes: Vec<_> = arrival.children.iter().map(|n| (n.visible, n.title.as_str())).collect();
    assert_eq!(scenes, vec![(false, "The Kitchen"), (false, "The Hallway")]);
}

#[test]
fn metadata_parsed_but_not_prose() {
    let root = parse(SAMPLE);
    let kitchen = &root.children[0].children[0].children[0];
    assert_eq!(
        kitchen.meta,
        vec![
            ("time".to_string(), "dawn".to_string()),
            ("pov".to_string(), "Alice".to_string()),
            ("characters".to_string(), "Alice, Bob".to_string()),
        ]
    );
    // "Bob said: nothing." must stay prose, not become metadata.
    let has_bob = kitchen.body.iter().any(|b| match b {
        Block::Para(spans) => spans.iter().any(|s| matches!(s, Inline::Text(t) if t.contains("Bob said: nothing"))),
        _ => false,
    });
    assert!(has_bob, "prose colon line was wrongly eaten as metadata");
}

#[test]
fn criticmarkup_scanned() {
    let root = parse(SAMPLE);
    let kitchen = &root.children[0].children[0].children[0];
    let first_para = kitchen.body.iter().find_map(|b| match b {
        Block::Para(s) => Some(s),
        _ => None,
    }).unwrap();
    assert!(first_para.iter().any(|s| matches!(s, Inline::Insert(cs) if *cs == txt("Steam rose from the kettle."))));
    assert!(first_para.iter().any(|s| matches!(s, Inline::Sub { old, new } if *old == txt("grey") && *new == txt("pale with morning"))));
    assert!(first_para.iter().any(|s| matches!(s, Inline::Comment(_))));
}

#[test]
fn manuscript_hides_invisible_and_resolves_markup() {
    let out = render(&parse(SAMPLE), View::Manuscript);
    // Visible chapter title present; scene title absent.
    assert!(out.contains("Chapter 1"));
    assert!(!out.contains("The Kitchen"));
    // Insertion accepted, substitution applied, deletion + comments gone.
    assert!(out.contains("Steam rose from the kettle."));
    assert!(out.contains("pale with morning"));
    assert!(!out.contains("This line gets cut."));
    assert!(!out.contains("too early in the timeline"));
    assert!(!out.contains("remember to seed"));
    // Meta never prints.
    assert!(!out.contains("pov"));
}

#[test]
fn outline_lists_every_heading() {
    let out = render(&parse(SAMPLE), View::Outline);
    for h in ["Chapter 1", "The Arrival", "The Kitchen", "The Hallway", "Chapter 2", "Departure"] {
        assert!(out.contains(h), "outline missing {h}");
    }
}

// A single Text span wrapped as an inline sequence (the common operand shape).
fn txt(s: &str) -> Vec<Inline> {
    vec![Inline::Text(s.to_string())]
}

// Parse a bare paragraph and return its inline spans.
fn inlines(text: &str) -> Vec<Inline> {
    let root = parse(text);
    match &root.body[0] {
        Block::Para(spans) => spans.clone(),
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

#[test]
fn spaced_asterisks_are_literal() {
    // The math case: every * has spaces both sides -> no emphasis.
    let spans = inlines("5 * 3 = 15 and 2 * 4 = 8");
    assert_eq!(spans, vec![Inline::Text("5 * 3 = 15 and 2 * 4 = 8".to_string())]);
}

#[test]
fn emphasis_hugs_and_ignores_trailing_punctuation() {
    // Closing * is preceded by 't'; the period sits outside the span.
    let spans = inlines("The dog was *fast*.");
    assert_eq!(
        spans,
        vec![
            Inline::Text("The dog was ".to_string()),
            Inline::Italic(txt("fast")),
            Inline::Text(".".to_string()),
        ]
    );
}

#[test]
fn bold_still_parses() {
    let spans = inlines("a **loud** noise");
    assert!(spans.contains(&Inline::Bold(txt("loud"))));
}

#[test]
fn markup_nests() {
    let spans = inlines("{+She was **furious**.}");
    let insert = spans
        .iter()
        .find_map(|s| match s {
            Inline::Insert(cs) => Some(cs),
            _ => None,
        })
        .expect("insertion span");
    assert_eq!(
        insert,
        &vec![
            Inline::Text("She was ".to_string()),
            Inline::Bold(txt("furious")),
            Inline::Text(".".to_string()),
        ]
    );
    // Manuscript accepts the insertion and keeps the nested bold.
    let out = render(&parse("{+She was **furious**.}"), View::Manuscript);
    assert!(out.contains("She was **furious**."));
}

#[test]
fn leading_and_trailing_asterisks_stay_literal() {
    // "* " (space after) can't open; " *" (space before) can't close.
    let spans = inlines("* not a bullet *");
    assert_eq!(spans, vec![Inline::Text("* not a bullet *".to_string())]);
}

#[test]
fn backslash_escapes_markers() {
    // \* -> literal asterisk, no emphasis; \{ -> literal brace.
    let spans = inlines(r"a \*star\* and a \{brace");
    let joined: String = spans
        .iter()
        .map(|s| match s {
            Inline::Text(t) => t.clone(),
            other => panic!("unexpected span {other:?}"),
        })
        .collect();
    assert_eq!(joined, "a *star* and a {brace");
}

fn slice(src: &str, s: ink_core::Span) -> String {
    src.chars().skip(s.start).take(s.end - s.start).collect()
}

#[test]
fn spans_map_back_to_source() {
    let root = parse(SAMPLE);
    let ch1 = &root.children[0];
    let ch2 = &root.children[1];

    // heading_span isolates the heading line.
    assert_eq!(slice(SAMPLE, ch2.heading_span), "# Chapter 2");

    // Chapter 1's subtree covers both scenes but stops before Chapter 2.
    let ch1_text = slice(SAMPLE, ch1.node_span);
    assert!(ch1_text.starts_with("# Chapter 1"));
    assert!(ch1_text.contains("The Kitchen"));
    assert!(ch1_text.contains("The Hallway"));
    assert!(!ch1_text.contains("Chapter 2"));

    // Sibling spans are contiguous: ch1 ends exactly where ch2 begins.
    assert_eq!(ch1.node_span.end, ch2.node_span.start);
    // Root covers the whole document.
    assert_eq!(root.node_span, ink_core::Span { start: 0, end: SAMPLE.chars().count() });
}

#[test]
fn spans_are_char_offsets_not_bytes() {
    // Non-ASCII before a heading: byte offsets would overshoot, char offsets
    // stay correct.
    let src = "# Café\n\nStuff.\n\n# Zwei\n";
    let root = parse(src);
    assert_eq!(slice(src, root.children[1].heading_span), "# Zwei");
}

#[test]
fn illegal_level_jump_is_clamped() {
    // Jump from level 1 straight to level 4 clamps to level 2.
    let root = parse("# Top\n\n~~~~ Deep\n");
    let deep = &root.children[0].children[0];
    assert_eq!(deep.title, "Deep");
    assert_eq!(deep.level, 2);
}

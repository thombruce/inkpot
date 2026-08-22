use ink_core::{parse, render, word_count, Block, Inline, View, Visibility};

const SAMPLE: &str = include_str!("../../../examples/sample.ink");

#[test]
fn tree_shape_and_nesting() {
    let root = parse(SAMPLE);
    // Two top-level visible chapters.
    assert_eq!(root.children.len(), 2);
    let ch1 = &root.children[0];
    assert_eq!(ch1.title, "Chapter 1");
    assert_eq!(ch1.visibility, Visibility::Visible);
    assert_eq!(ch1.level, 1);

    // Chapter 1 -> The Arrival (##).
    let arrival = &ch1.children[0];
    assert_eq!(arrival.title, "The Arrival");
    assert_eq!(arrival.level, 2);

    // The Arrival holds two invisible scenes as peers.
    let scenes: Vec<_> = arrival.children.iter().map(|n| (n.visibility, n.title.as_str())).collect();
    assert_eq!(
        scenes,
        vec![
            (Visibility::Scene, "The Kitchen"),
            (Visibility::Scene, "The Hallway"),
        ]
    );
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
fn leading_frontmatter_is_document_metadata() {
    // A `key: value` block at the top populates the root, not a chapter or body.
    let root = parse("title: The Book\nauthor: Thom\n\n# Chapter One\n\nBody.\n");
    assert_eq!(
        root.meta,
        vec![
            ("title".to_string(), "The Book".to_string()),
            ("author".to_string(), "Thom".to_string()),
        ]
    );
    assert_eq!(root.children.len(), 1);
    assert_eq!(root.children[0].title, "Chapter One");
    // Front matter round-trips through the edit view (root meta re-serialized).
    assert!(render(&root, View::Edit).starts_with("title: The Book\nauthor: Thom\n"));

    // Front matter must start at line 1 — a leading blank closes the zone first.
    assert!(parse("\ntitle: x\n\n# H\n").meta.is_empty());
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
    // Visible headings render as Markdown ATX headings (depth = marker count);
    // scene titles are absent (scenes contribute body only).
    assert!(out.contains("# Chapter 1"));
    assert!(out.contains("## The Arrival"));
    assert!(!out.contains("The Kitchen"));
    // Insertion accepted, substitution applied, deletion + comments gone.
    assert!(out.contains("Steam rose from the kettle."));
    assert!(out.contains("pale with morning"));
    assert!(!out.contains("This line gets cut."));
    assert!(!out.contains("too early in the timeline"));
    assert!(!out.contains("remember to seed"));
    // Meta never prints.
    assert!(!out.contains("pov"));
    // Exactly one trailing newline, no dangling blank line.
    assert!(out.ends_with("noon.\n") && !out.ends_with("\n\n"));
}

#[test]
fn word_count_counts_manuscript_prose_only() {
    // Heading titles excluded; scene body counts; `%` subtree contributes 0.
    let doc = "# Chapter\n\nThree plain words.\n\n~ Scene\n\nTwo more.\n\n% Cut\n\nHidden words here.\n";
    assert_eq!(word_count(&parse(doc)), 5); // 3 + 2, excluded drops

    // CriticMarkup resolves before counting: insert adds, delete drops, sub uses new.
    let cm = "# H\n\nkeep {+added} {-removed here} {~old~new} end.\n";
    assert_eq!(word_count(&parse(cm)), 4); // keep, added, new, end
}

#[test]
fn outline_lists_every_heading() {
    let out = render(&parse(SAMPLE), View::Outline);
    for h in ["Chapter 1", "The Arrival", "The Kitchen", "The Hallway", "Chapter 2", "Departure"] {
        assert!(out.contains(h), "outline missing {h}");
    }
}

#[test]
fn codex_indexes_excluded_subtrees_with_metadata() {
    let src = "# Chapter\n\nProse.\n\n% Characters\n\n%% Alice\nrole: lead\nhome: London\n\n% Timeline\n\n%% 1989\nlocation: London\n";
    let out = render(&parse(src), View::Codex);
    // Excluded sections + nested entries + their metadata are indexed.
    for s in ["Characters", "Alice", "role: lead", "home: London", "Timeline", "1989", "location: London"] {
        assert!(out.contains(s), "codex missing {s}\n---\n{out}");
    }
    // Non-excluded content stays out of the codex.
    assert!(!out.contains("Chapter"), "codex leaked visible heading");
    assert!(!out.contains("Prose"), "codex leaked prose");
}

#[test]
fn codex_html_nests_entities_and_metadata() {
    let src = "% Characters\n\n%% Alice\nrole: lead\n\nBaker & insomniac.\n";
    let html = ink_core::render_codex_html(&parse(src));
    assert!(html.contains("<section class=\"codex-section\">"));
    assert!(html.contains("<h2>Characters</h2>"));
    assert!(html.contains("<article class=\"entity\"><h3>Alice</h3>"));
    assert!(html.contains("<dt>role</dt><dd>lead</dd>"));
    assert!(html.contains("<p>Baker &amp; insomniac.</p>"), "prose escaped: {html}");
}

#[test]
fn codex_scopes_repeated_notes_by_visible_ancestors() {
    // Two `%% Synopsis`, one per chapter: each renders under its chapter's scope
    // so they read distinctly instead of as two bare "Synopsis" lines.
    let src = "# Chapter 1\n\n%% Synopsis\n\nA.\n\n# Chapter 2\n\n%% Synopsis\n\nB.\n\n% Characters\n";
    let text = render(&parse(src), View::Codex);
    assert!(text.contains("[Chapter 1]"), "missing chapter-1 scope: {text}");
    assert!(text.contains("[Chapter 2]"), "missing chapter-2 scope: {text}");
    let html = ink_core::render_codex_html(&parse(src));
    assert!(html.contains("<div class=\"codex-scope\">Chapter 1</div>"), "no html scope: {html}");
    // A root-level `%` (Characters) carries no scope breadcrumb.
    assert!(!html.contains("<div class=\"codex-scope\">Characters"), "root entity mis-scoped: {html}");
    // Nested scope joins ancestors.
    let nested = "# Frankenstein\n\n## Chapter 1\n\n%%% Synopsis\n\nX.\n";
    let nhtml = ink_core::render_codex_html(&parse(nested));
    assert!(nhtml.contains("<div class=\"codex-scope\">Frankenstein / Chapter 1</div>"), "nested scope wrong: {nhtml}");
}

#[test]
fn codex_resolves_metadata_refs_and_backlinks() {
    // A scene names Alice; the timeline entry names London and Alice. Both are
    // `%` entities, so those values become links and the entities get backlinks.
    let src = "~~~ The Kitchen\ncharacters: Alice\n\nProse.\n\n% Characters\n\n%% Alice\nhome: London\n\n% Locations\n\n%% London\n\n% Timeline\n\n%% 1989\nlocation: London\ncharacters: Alice\n";
    let html = ink_core::render_codex_html(&parse(src));
    // Alice's `home: London` resolves to the London entity (a link).
    assert!(html.contains("<dt>home</dt><dd><a class=\"ref\" data-jump="), "home not linked: {html}");
    assert!(html.contains(">London</a>"), "London not linked: {html}");
    // London is referenced by Alice and 1989; Alice by the scene and 1989.
    assert!(html.contains("Referenced by"), "no backlinks: {html}");
    assert!(html.contains(">The Kitchen</a>"), "scene backlink missing: {html}");
    assert!(html.contains(">1989</a>"), "timeline backlink missing: {html}");
    // Case-insensitive, unknown values stay plain text.
    let plain = ink_core::render_codex_html(&parse("% C\n\n%% Bob\nrole: villain\n"));
    assert!(plain.contains("<dd>villain</dd>"), "non-entity value should not link: {plain}");
}

#[test]
fn wikilink_parses_prints_name_and_round_trips() {
    let root = parse("She saw [[Alice]] leave.\n");
    let Block::Para(spans) = &root.body[0] else { panic!("expected para") };
    assert!(spans.contains(&Inline::Link("Alice".to_string())), "no link inline: {spans:?}");
    // Manuscript prints the bare name (the link is part of the prose).
    assert_eq!(render(&root, View::Manuscript), "She saw Alice leave.\n");
    // Edit round-trips the brackets.
    assert!(render(&root, View::Edit).contains("She saw [[Alice]] leave."));
    // An escaped or unclosed bracket stays literal.
    assert!(render(&parse("a \\[[b"), View::Manuscript).contains("a [[b"));
    assert_eq!(render(&parse("open [[ only\n"), View::Manuscript), "open [[ only\n");
}

#[test]
fn wikilink_in_prose_backlinks_to_entity() {
    let src = "~~~ Scene\n\nAcross the room, [[Alice]] said nothing.\n\n% C\n\n%% Alice\n";
    let html = ink_core::render_codex_html(&parse(src));
    assert!(html.contains("Referenced by"), "no backlink from prose: {html}");
    assert!(html.contains(">Scene</a>"), "scene not backlinked: {html}");
}

#[test]
fn interpolation_resolves_numbering_and_metadata() {
    // number = 1-based position among non-excluded siblings; total = their count.
    let doc = "# Chapter {{number}}\n\n# Chapter {{number}} of {{total}}\n";
    let m = render(&parse(doc), View::Manuscript);
    assert!(m.contains("# Chapter 1\n"), "{m}");
    assert!(m.contains("# Chapter 2 of 2\n"), "{m}");

    // Arithmetic: the countdown 1-N..0 from `number - total`.
    let cd = "# {{number - total}}\n\n# {{number - total}}\n";
    let m2 = render(&parse(cd), View::Manuscript);
    assert!(m2.contains("# -1\n") && m2.contains("# 0\n"), "{m2}");
    // Precedence + parens + unary minus.
    assert!(render(&parse("# {{-1 * (total - number)}}\n\n# x\n"), View::Manuscript).contains("# -1\n"));
    // Overflow is left verbatim, not a panic (debug) or wrapped garbage (release).
    assert!(render(&parse("# {{9999999999 * 9999999999}}\n"), View::Manuscript)
        .contains("{{9999999999 * 9999999999}}"));

    // A `%` sibling is outside numbering (manuscript-authoritative).
    let ex = "# One {{number}}\n\n% Note\n\n# Two {{number}}\n";
    let m3 = render(&parse(ex), View::Manuscript);
    assert!(m3.contains("# One 1\n") && m3.contains("# Two 2\n"), "{m3}");

    // Front-matter var cascades into prose; an unknown var stays raw.
    let meta = "place: New York\n\n# Home\n\nThere in {{place}}, not {{gone}}.\n";
    let m4 = render(&parse(meta), View::Manuscript);
    assert!(m4.contains("There in New York, not {{gone}}."), "{m4}");

    // Metadata used in arithmetic; a scene's own meta wins over front matter.
    let arith = "base: 10\n\n# H\n\n{{base + number}}\n";
    assert!(render(&parse(arith), View::Manuscript).contains("11"), "meta arithmetic");

    // Heading `\{{` escapes to a literal (title is not scanned by the parser).
    let esc = "# Literal \\{{number}}\n\n# x\n";
    assert!(render(&parse(esc), View::Manuscript).contains("# Literal {{number}}\n"), "escape");

    // A metadata key named like a built-in does not shadow it (built-ins win).
    let shadow = "total: 99\n\n# H {{total}}\n\n# H2\n";
    assert!(render(&parse(shadow), View::Manuscript).contains("# H 2\n"), "built-in wins");

    // Edit view round-trips the raw source, unresolved.
    assert!(render(&parse("# Chapter {{number}}\n"), View::Edit).contains("# Chapter {{number}}"));
}

#[test]
fn interpolation_resolves_in_outline_and_codex() {
    // resolve_titles gives the same resolved titles the views show, keyed by offset.
    let titles =
        ink_core::resolve_titles(&parse("# Chapter {{number}}\n\n# Chapter {{number}} of {{total}}\n"));
    assert!(titles.values().any(|t| t == "Chapter 1"), "{titles:?}");
    assert!(titles.values().any(|t| t == "Chapter 2 of 2"), "{titles:?}");

    // Codex titles resolve, with the front-matter cascade reaching a `%` entry.
    let cx = render(&parse("place: Paris\n\n% Places\n\n%% Home {{place}}\n"), View::Codex);
    assert!(cx.contains("Home Paris"), "{cx}");

    // Codex HTML resolves metadata interpolation in a note's body.
    let html = ink_core::render_codex_html(&parse(
        "place: Paris\n\n% Places\n\n%% Home\n\nSet in {{place}}.\n",
    ));
    assert!(html.contains("Set in Paris."), "{html}");

    // A backlink label from a `{{number}}` heading shows the resolved title.
    let bl = ink_core::render_codex_html(&parse(
        "# Chapter {{number}}\n\n[[Alice]] appears.\n\n% C\n\n%% Alice\n",
    ));
    assert!(bl.contains(">Chapter 1</a>"), "backlink label unresolved: {bl}");
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

#[test]
fn html_manuscript_renders_tags_and_escapes() {
    let src = "# Ch <One>\n\n~~ Scene\nk: v\n\nA {+bold **word**} & a *slant*. {/note}\n";
    let html = ink_core::render_html(&parse(src));
    assert!(html.contains("<h1>Ch &lt;One&gt;</h1>"), "{html}");
    assert!(html.contains("<strong>word</strong>"));
    assert!(html.contains("<em>slant</em>"));
    assert!(html.contains("&amp;"));
    // Scene heading, metadata, and comments never appear.
    assert!(!html.contains("Scene"));
    assert!(!html.contains("note"));
    assert!(!html.contains("k:"));
}

#[test]
fn excluded_section_drops_from_manuscript_but_stays_in_tree() {
    let src = "# Kept\n\nvisible prose.\n\n% Cut draft\n\nsecret notes.\n\n## nested kept\n\nstill secret.\n";
    let root = parse(src);
    // The % section is a normal top-level node in the tree.
    let cut = &root.children[1];
    assert_eq!(cut.title, "Cut draft");
    assert_eq!(cut.visibility, Visibility::Excluded);
    assert_eq!(cut.children.len(), 1); // the nested "## nested kept" rides along

    // Manuscript (plain + HTML) omits the whole excluded subtree.
    let manu = render(&root, View::Manuscript);
    assert!(manu.contains("visible prose."));
    assert!(!manu.contains("secret notes."));
    assert!(!manu.contains("Cut draft"));
    assert!(!manu.contains("still secret."));
    let html = ink_core::render_html(&root);
    assert!(!html.contains("secret"));

    // Outline still shows it, with the % sigil.
    let out = render(&root, View::Outline);
    assert!(out.contains("% Cut draft"));
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

#[test]
fn same_depth_headings_at_start_are_siblings() {
    // Opening at `##` must not demote the first to level 1 and nest the rest
    // beneath it — the clamp only applies against a real heading parent.
    let root = parse("## Chapter 1\n\n## Chapter 2\n\n## Chapter 3\n");
    assert_eq!(root.children.len(), 3);
    assert!(root.children.iter().all(|c| c.level == 2));
    assert_eq!(root.children[1].title, "Chapter 2");
}

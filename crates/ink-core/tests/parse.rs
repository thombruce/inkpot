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
    assert!(first_para.iter().any(|s| matches!(s, Inline::Insert(t) if t == "Steam rose from the kettle.")));
    assert!(first_para.iter().any(|s| matches!(s, Inline::Sub { old, new } if old == "grey" && new == "pale with morning")));
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

#[test]
fn illegal_level_jump_is_clamped() {
    // Jump from level 1 straight to level 4 clamps to level 2.
    let root = parse("# Top\n\n~~~~ Deep\n");
    let deep = &root.children[0].children[0];
    assert_eq!(deep.title, "Deep");
    assert_eq!(deep.level, 2);
}

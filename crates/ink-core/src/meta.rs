//! Reserved metadata keys — the single source of truth for keys that carry
//! defined meaning in `ink-core`, as opposed to the author's free-form
//! vocabulary (scene `time`, `pov`, `characters`, … — parsed and preserved,
//! but given no special meaning here).
//!
//! The reserved vocabulary, and where each group is consumed:
//!
//! - **Core** — `id`: a rename-proof handle that names its own section. It is a
//!   *self-naming* key (see [`is_self_naming`]): excluded from outgoing
//!   references/backlinks and rendered plain, never as a `[[link]]`. This is the
//!   only reserved key with behavior in `ink-core` today.
//! - **View keys** — `time`, `location`, `characters`, `coords`, `map`, `pov`:
//!   read by the app's structured views (timeline/map/time-scrub, see #45). A
//!   view key earns a constant here once core grows behavior for it: `time`,
//!   `location`, `characters`, `coords`, and `map` have one; `pov` stays
//!   free-form until read.
//! - **Export keys** — `title`, `author`, `contact`, `byline`: front-matter
//!   metadata for manuscript export (Shunn/pandoc, see #23). Likewise no core
//!   behavior yet.
//!
//! Only self-naming keys need a predicate here, because that is the only
//! reserved behavior core implements. New reserved behavior gets a named
//! predicate here rather than a scattered `k == "…"` literal.

/// The rename-proof section handle. Names its own `%` codex entity so a
/// `[[link]]` or metadata reference survives a title rename.
pub const ID: &str = "id";

/// When a heading happens, in story time. The timeline view orders headings by
/// this value; ISO dates (`YYYY-MM-DD`) sort chronologically as plain strings.
pub const TIME: &str = "time";

/// A location entity's geographic position, `lat, lon` (decimal degrees). The
/// map view places a marker for each entity that has a parseable pair.
pub const COORDS: &str = "coords";

/// Which map a location sits on — e.g. `Earth`, `Mars`, `Moon` (folded). Empty
/// means the default world (Earth). The map view groups markers by this and
/// switches the tile backdrop per world.
pub const MAP: &str = "map";

/// Where a scene takes place — a location entity's name. The time-scrub joins it
/// to that location's [`COORDS`] to place characters on the map.
pub const LOCATION: &str = "location";

/// Who is present in a scene — a comma-separated list of character names. The
/// time-scrub places each at the scene's [`LOCATION`] as the cursor passes it.
pub const CHARACTERS: &str = "characters";

/// Characters whose *last* scene this is — a comma-separated list (death,
/// departure, written out). The time-scrub shows them in this scene, then drops
/// them from the next one on, so the marker sits on the character's finale (and
/// travels with it if the scene is reordered). Naming one again in a later
/// [`CHARACTERS`] re-adds them (a flashback or fake-out).
pub const EXITS: &str = "exits";

// --- Export front matter (document-level, on the root node) -----------------
// Read by the Shunn manuscript export (#23) off the root's `meta`. (A future
// multi-file "book" export will read a work's values from the `Inkpot` marker's
// front matter instead — not yet implemented; see #74/#80.)

/// The work's title. Centered on the Shunn title page; also seeds the running
/// header keyword when none is given.
pub const TITLE: &str = "title";

/// The author's legal name — the byline default, and the source of the running
/// header surname (its last whitespace-separated word).
pub const AUTHOR: &str = "author";

/// The author's contact block (name/address/email/phone), top-left on the Shunn
/// first page. A multiline front-matter value: indented continuation lines each
/// become their own line.
pub const CONTACT: &str = "contact";

/// The name to publish under (pen name), centered under the title. Falls back to
/// [`AUTHOR`] when absent.
pub const BYLINE: &str = "byline";

/// Does this metadata key *name its own section* rather than reference another
/// entity? Such keys are excluded from backlinks and rendered as plain text,
/// never resolved as a `[[link]]`. Today the only one is [`ID`].
pub fn is_self_naming(key: &str) -> bool {
    key == ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_naming_is_id_only() {
        assert!(is_self_naming("id"));
        assert!(!is_self_naming("time"));
        assert!(!is_self_naming("location"));
        assert!(!is_self_naming("characters"));
        // case-sensitive: keys are stored verbatim, folding happens at lookup.
        assert!(!is_self_naming("ID"));
    }
}

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
//! - **View keys** — `time`, `location`, `characters`, `pov`: free-form as far
//!   as core is concerned; the app's structured views (timeline/map, see #45)
//!   read them. No core behavior, so no core constant until they earn one.
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

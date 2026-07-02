//! Test fixture helpers for deterministic core scenarios.
//!
//! Phase 2 will add board-specific fixture parsing here. Phase 1 keeps this
//! module intentionally generic so the workspace has a stable place for compact
//! text fixtures without introducing game rules early.

/// A named text fixture used by deterministic core tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextFixture<'a> {
    /// Human-readable fixture name used in test diagnostics.
    pub name: &'a str,
    /// Fixture contents in the compact text format owned by the calling module.
    pub contents: &'a str,
}

impl<'a> TextFixture<'a> {
    /// Creates a fixture wrapper without interpreting its contents.
    #[must_use]
    pub const fn new(name: &'a str, contents: &'a str) -> Self {
        Self { name, contents }
    }
}

#[cfg(test)]
mod tests {
    use super::TextFixture;

    #[test]
    fn fixture_wrapper_preserves_name_and_contents() {
        let fixture = TextFixture::new("empty-board", "..........\n");

        assert_eq!(fixture.name, "empty-board");
        assert_eq!(fixture.contents, "..........\n");
    }
}

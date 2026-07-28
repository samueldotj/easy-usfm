//! USFM versions, and how a document's version is discovered.
//!
//! Severity depends on two of them (PRODUCT §9). A marker deprecated at or
//! before the **target** version is a warning, because the author is writing
//! for that version. A marker introduced *after* the **document's** version is
//! information, because the document says it is older than the construct it
//! contains — which is usually a stale `\usfm` line rather than a mistake, and
//! so is worth mentioning and not worth complaining about.

/// A USFM version, as `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
}

impl Version {
    pub const V1_0: Self = Self::new(1, 0);
    pub const V2_0: Self = Self::new(2, 0);
    pub const V3_0: Self = Self::new(3, 0);
    pub const V3_1: Self = Self::new(3, 1);

    /// What the engine targets, and the default when a document says nothing.
    pub const TARGET: Self = Self::V3_1;

    /// What a document is assumed to be when it carries no `\usfm` line.
    ///
    /// 3.0 rather than 3.1, because `\usfm` became available in 3.0 — a file
    /// without one predates the marker or chose not to use it, and assuming
    /// the newest version would suppress exactly the diagnostics that help.
    pub const ASSUMED: Self = Self::V3_0;

    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }

    /// Parses `3.1`, `3`, or `3.1.2` — the last taking only the first two
    /// components, since USFM has never used a third.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let mut parts = text.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().map_or(Some(0), |m| m.parse().ok())?;
        Some(Self::new(major, minor))
    }

    /// The version a document declares, if it declares one.
    ///
    /// `\usfm 3.0` is line-initial and belongs in the header, so only the
    /// text before the first chapter is searched. Scanning the whole document
    /// would find the marker quoted in a translator's note.
    pub fn detect(source: &str) -> Option<Self> {
        for line in source.lines() {
            if line.starts_with("\\c ") || line == "\\c" {
                break;
            }
            if let Some(rest) = line.strip_prefix("\\usfm") {
                if rest.starts_with([' ', '\t']) {
                    return Self::parse(rest);
                }
            }
        }
        None
    }

    /// The declared version, or the assumed one.
    pub fn of(source: &str) -> Self {
        Self::detect(source).unwrap_or(Self::ASSUMED)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse_and_order() {
        assert_eq!(Version::parse("3.1"), Some(Version::V3_1));
        assert_eq!(Version::parse("3"), Some(Version::V3_0));
        assert_eq!(Version::parse(" 2.0 "), Some(Version::V2_0));
        assert_eq!(Version::parse("3.1.2"), Some(Version::V3_1));
        assert_eq!(Version::parse("nonsense"), None);

        assert!(Version::V2_0 < Version::V3_0);
        assert!(Version::V3_0 < Version::V3_1);
    }

    #[test]
    fn a_declared_version_is_found_in_the_header() {
        assert_eq!(
            Version::detect("\\id GEN\n\\usfm 3.0\n\\c 1\n"),
            Some(Version::V3_0)
        );
    }

    #[test]
    fn a_document_with_no_declaration_is_assumed_rather_than_guessed_newest() {
        assert_eq!(Version::detect("\\id GEN\n\\c 1\n"), None);
        assert_eq!(Version::of("\\id GEN\n\\c 1\n"), Version::ASSUMED);
    }

    #[test]
    fn the_marker_is_not_looked_for_past_the_first_chapter() {
        // A translator quoting "\usfm 2.0" in a note is not a declaration.
        assert_eq!(Version::detect("\\id GEN\n\\c 1\n\\p\n\\usfm 2.0\n"), None);
    }

    #[test]
    fn markers_that_merely_begin_with_usfm_are_not_declarations() {
        assert_eq!(Version::detect("\\id GEN\n\\usfmx 9.9\n"), None);
    }
}

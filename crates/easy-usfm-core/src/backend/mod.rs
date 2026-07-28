//! The parser, and the only place in this crate that knows which one it is.
//!
//! ADR-001 adopts `usfm3` behind a facade, pinned exactly, on the reasoning
//! that a five-month-old crate with one maintainer is a real hazard at the
//! centre of the product and the facade makes it swappable. That control only
//! holds if the containment is actual, so `usfm3` is named here and nowhere
//! else — `tests/facade_boundary.rs` fails the build if it appears in any
//! other file.
//!
//! Everything crossing out of this module is one of our own types.

mod diagnostics;
mod tree;

use crate::{Diagnostic, Node};

/// A parsed document, holding the parser's own staged representation.
///
/// The staging is real — the parser uses a `OnceCell` per stage, so asking for
/// diagnostics does not pay for the document tree and vice versa. ARCHITECTURE
/// §8.1 maps our tiers onto it.
pub(crate) struct Backend {
    parsed: usfm3::ParsedDocument,
}

impl Backend {
    /// Holding a second copy of the source is unavoidable here.
    ///
    /// ADR-001 notes that `parse` copies its input and recommends
    /// `parse_owned` instead, but `ParsedDocument` does not expose the source
    /// it swallowed, and ADR-003 makes our copy the authoritative one — so
    /// handing over the only copy is not an option. Two copies of a 2 MB file
    /// sits inside the memory budget (ARCHITECTURE §11, under 6x file size),
    /// and P0.4's incremental session removes the question by not reparsing
    /// whole documents.
    pub(crate) fn parse(source: &str) -> Self {
        Self {
            // Diagnostics are requested at parse time but computed lazily, so
            // this costs nothing until something asks for them.
            parsed: usfm3::parse(source, usfm3::ParseOptions { diagnostics: true }),
        }
    }

    /// The document's top-level nodes, converted to our model.
    pub(crate) fn tree(&self) -> Vec<Node> {
        tree::convert_document(self.parsed.ast(), self.parsed.source_map())
    }

    /// Diagnostics, converted to our codes.
    pub(crate) fn diagnostics(&self) -> Vec<Diagnostic> {
        self.parsed
            .diagnostics()
            .unwrap_or_default()
            .iter()
            .map(diagnostics::convert)
            .collect()
    }
}

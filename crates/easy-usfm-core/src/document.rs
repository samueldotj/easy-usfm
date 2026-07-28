//! The document — this crate's entry point.

use std::cell::OnceCell;

use crate::backend::Backend;
use crate::{ByteSpan, Char16Range, Diagnostic, Node, Utf16Mapper};

/// A USFM document: its source text, and what the engine has worked out about
/// it.
///
/// **The source is authoritative** (ADR-003). Everything else here is derived
/// from it and disposable, and this type never reconstructs source from the
/// tree — saving writes the buffer. Byte-exactness is therefore a property of
/// the architecture rather than of the parser, which is why adopting a parser
/// with a lossy serializer costs us nothing.
///
/// Derived state is computed on first use and cached. ARCHITECTURE §8.1 tiers
/// the work so the cheap path does not pay for the expensive one; asking for
/// diagnostics does not build the tree.
///
/// This is the whole-document path. Reparsing one chapter instead of the whole
/// book is the incremental session, P0.4.
pub struct Document {
    source: String,
    backend: Backend,
    content: OnceCell<Vec<Node>>,
    diagnostics: OnceCell<Vec<Diagnostic>>,
    mapper: OnceCell<Utf16Mapper>,
}

impl Document {
    /// Parses `source`.
    ///
    /// Cheap: the parser stages its work behind a cell per stage, so this
    /// costs little until something is asked of the result.
    pub fn parse(source: impl Into<String>) -> Self {
        let source = source.into();
        let backend = Backend::parse(&source);

        Self {
            source,
            backend,
            content: OnceCell::new(),
            diagnostics: OnceCell::new(),
            mapper: OnceCell::new(),
        }
    }

    /// The source text, exactly as it was given.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The top-level nodes of the document tree.
    ///
    /// A list rather than a single root because that is USJ's shape: the root
    /// object holds a `content` array, and giving it a synthetic parent would
    /// be a divergence from the model with nothing to show for it.
    pub fn content(&self) -> &[Node] {
        self.content.get_or_init(|| self.backend.tree())
    }

    /// Every node in the document, parents before children.
    pub fn descendants(&self) -> impl Iterator<Item = &Node> {
        self.content().iter().flat_map(Node::descendants)
    }

    /// What is wrong with the document, in source order.
    ///
    /// Never a reason to prevent saving — users must be able to save
    /// incomplete work (PRODUCT §9).
    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.diagnostics.get_or_init(|| {
            let mut diagnostics = self.backend.diagnostics();
            diagnostics.sort_by_key(|diagnostic| diagnostic.span.start);
            diagnostics
        })
    }

    /// The byte-to-Char16 mapper for this document's source.
    ///
    /// Built on first use, because a document that is only being parsed never
    /// needs it — the conversion is a boundary concern, not a parsing one.
    pub fn mapper(&self) -> &Utf16Mapper {
        self.mapper.get_or_init(|| Utf16Mapper::new(&self.source))
    }

    /// Converts a span into the coordinate space everything outside this crate
    /// speaks.
    ///
    /// This is the only way a span reaches the frontend, and going through the
    /// document rather than through [`Utf16Mapper`] directly removes the one
    /// way that conversion can be got wrong: the source and the index cannot
    /// disagree, because the document owns both.
    ///
    /// `None` only if the span is malformed in a way `Utf16Mapper` refuses.
    pub fn to_char16(&self, span: &ByteSpan) -> Option<Char16Range> {
        self.mapper().to_char16_range(&self.source, span)
    }
}

impl std::fmt::Debug for Document {
    /// Prints the shape rather than the content — a 2 MB source in a panic
    /// message helps nobody.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("source_len", &self.source.len())
            .field("parsed", &self.content.get().is_some())
            .finish()
    }
}

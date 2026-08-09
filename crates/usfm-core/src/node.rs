//! The document tree: the USJ content model, extended with source location.
//!
//! ADR-004 adopts USJ rather than a preview-shaped tree of our own, so
//! structural questions have documented answers and the three-way differential
//! oracle (P0.10) can diff structurally. Node kinds and attribute keys
//! therefore use the specification's vocabulary, even where a more idiomatic
//! Rust name suggests itself.
//!
//! Two deliberate extensions, both additive — strip `span` and `raw` and what
//! remains is valid USJ:
//!
//! - `span`, because every feature connecting the preview to the text needs it
//!   (ADR-003).
//! - `raw`, because the published model describes valid documents and we must
//!   also hold invalid ones. Malformed content has to survive editing and
//!   saving; dropping it is exactly what ADR-003 exists to prevent.

use crate::span::ByteSpan;

/// A node in the document tree.
///
/// Uniform rather than an enum per kind: the parser emits sixteen kinds that
/// differ mainly in which attributes they carry, and USJ itself models them as
/// one object shape keyed by `type`. Matching that shape keeps the oracle
/// comparison direct and the preview's fallthrough arm honest.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: NodeKind,

    /// The marker that introduced this node, without its backslash — `p`,
    /// `q1`, `zaln-s`. `None` for kinds the specification does not mark:
    /// text, optional breaks, tables, and references.
    pub marker: Option<Marker>,

    /// Attributes in specification vocabulary.
    ///
    /// This carries what USJ models as node properties as well as what USFM
    /// writes after `|` — a chapter's `number` and `sid` sit here alongside a
    /// figure's `src`. USJ makes the same choice, and it keeps every kind one
    /// shape.
    pub attributes: Vec<Attribute>,

    /// Where this node sits in the source.
    ///
    /// `None` for text and optional breaks. That is not an oversight and not
    /// a placeholder: the parser keeps source locations in a tree parallel to
    /// the syntax tree, and it populates that tree for structural nodes only —
    /// text leaves are recorded with no span and no CST anchor. Modelling the
    /// absence honestly is what stops a fabricated zero span from propagating
    /// into click-to-source (P3.6), where it would put the cursor at the top of
    /// the file and look like a rendering bug.
    ///
    /// A text leaf's position is still recoverable, and [`crate::TextCursor`]
    /// recovers it — the lowering copies text through verbatim, so a walk in
    /// document order finds each run where it is. That is done at the boundary
    /// that needs it rather than written back here, because a span this field
    /// did not get from the parser is a different kind of fact from one it did.
    pub span: Option<ByteSpan>,

    /// Index of the CST node this was lowered from, where the parser recorded
    /// one. The route by which the missing spans above become recoverable.
    pub anchor_cst: Option<usize>,

    pub children: Vec<Node>,

    /// Text content, for [`NodeKind::Text`] only.
    pub text: Option<String>,

    /// Source preserved verbatim because it could not be classified. Rendered
    /// as an inline placeholder rather than dropped.
    pub raw: Option<ByteSpan>,
}

impl Node {
    /// A node of `kind` with everything optional left empty.
    pub fn new(kind: NodeKind) -> Self {
        Self {
            kind,
            marker: None,
            attributes: Vec::new(),
            span: None,
            anchor_cst: None,
            children: Vec::new(),
            text: None,
            raw: None,
        }
    }

    /// The first attribute with this key.
    pub fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.key == key)
            .map(|attribute| attribute.value.as_str())
    }

    /// This node and every descendant, parents before children.
    pub fn descendants(&self) -> impl Iterator<Item = &Node> {
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children.iter().rev());
            Some(node)
        })
    }
}

/// USJ node types.
///
/// Names and string forms follow the specification. `Unknown` is the
/// specification's own category for the `\z` namespace and unrecognized
/// markers, not our error case — content we could not classify at all is
/// carried on [`Node::raw`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Book,
    Chapter,
    Verse,
    Para,
    Char,
    Note,
    /// Milestone — `\qt-s\*`, `\ts-s\*`. USJ spells this `ms`.
    #[serde(rename = "ms")]
    Milestone,
    Figure,
    Sidebar,
    Periph,
    Table,
    #[serde(rename = "table:row")]
    TableRow,
    #[serde(rename = "table:cell")]
    TableCell,
    #[serde(rename = "ref")]
    Reference,
    #[serde(rename = "optbreak")]
    OptBreak,
    Unknown,
    Text,
}

impl NodeKind {
    /// The USJ `type` string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Book => "book",
            Self::Chapter => "chapter",
            Self::Verse => "verse",
            Self::Para => "para",
            Self::Char => "char",
            Self::Note => "note",
            Self::Milestone => "ms",
            Self::Figure => "figure",
            Self::Sidebar => "sidebar",
            Self::Periph => "periph",
            Self::Table => "table",
            Self::TableRow => "table:row",
            Self::TableCell => "table:cell",
            Self::Reference => "ref",
            Self::OptBreak => "optbreak",
            Self::Unknown => "unknown",
            Self::Text => "text",
        }
    }
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A marker name without its backslash.
///
/// Owned rather than borrowed from the source or interned. The parser interns
/// unrecognized marker names by leaking them, which is tolerable for a
/// one-shot CLI and not for an editor that reparses as you type — every
/// keystroke of `\zaln-s` would leak a distinct prefix. Owning our own copy at
/// the facade means nothing above this crate inherits that.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(transparent)]
pub struct Marker(String);

impl Marker {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this is a custom marker in the `\z` namespace, which the
    /// specification leaves open-ended by design.
    pub fn is_custom(&self) -> bool {
        self.0.starts_with('z')
    }
}

impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\\{}", self.0)
    }
}

/// A key-value attribute.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Attribute {
    pub key: String,
    pub value: String,
}

impl Attribute {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usj_type_strings_match_the_specification() {
        assert_eq!(NodeKind::Milestone.as_str(), "ms");
        assert_eq!(NodeKind::TableRow.as_str(), "table:row");
        assert_eq!(NodeKind::TableCell.as_str(), "table:cell");
        assert_eq!(NodeKind::Reference.as_str(), "ref");
        assert_eq!(NodeKind::OptBreak.as_str(), "optbreak");
    }

    #[test]
    fn serialized_kind_agrees_with_the_type_string() {
        for kind in [
            NodeKind::Book,
            NodeKind::Milestone,
            NodeKind::TableRow,
            NodeKind::TableCell,
            NodeKind::Reference,
            NodeKind::OptBreak,
            NodeKind::Text,
        ] {
            let json = serde_json::to_string(&kind).expect("kind serializes");
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
        }
    }

    #[test]
    fn markers_display_with_their_backslash() {
        assert_eq!(Marker::new("q1").to_string(), "\\q1");
        assert!(Marker::new("zaln-s").is_custom());
        assert!(!Marker::new("p").is_custom());
    }

    #[test]
    fn descendants_yields_parents_before_children() {
        let mut root = Node::new(NodeKind::Para);
        let mut child = Node::new(NodeKind::Char);
        child.children.push(Node::new(NodeKind::Text));
        root.children.push(child);
        root.children.push(Node::new(NodeKind::Verse));

        let kinds: Vec<_> = root.descendants().map(|node| node.kind).collect();
        assert_eq!(
            kinds,
            vec![
                NodeKind::Para,
                NodeKind::Char,
                NodeKind::Text,
                NodeKind::Verse
            ]
        );
    }
}

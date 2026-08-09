//! Putting back text the parser mistook for attributes.
//!
//! # The bug
//!
//! `usfm3`'s lexer treats **every** `|` as the start of an attribute block,
//! whatever encloses it. The builder then collects those attributes into a
//! local vector — and for a paragraph, `Node::Para` has nowhere to put them,
//! so `lower_para` drops the vector on the floor. The text goes with it,
//! silently and with no diagnostic:
//!
//! ```text
//! \v 11 before| after more words   →   [verse, " before"]
//! ```
//!
//! # Why it matters more than it looks
//!
//! USFM attributes are only meaningful inside a character marker closed with
//! `\marker*`. A `|` in running text is punctuation — and specifically it is
//! the **danda**, the full stop of Sanskrit-derived scripts. Several published
//! Indic translations write every sentence that way, so this is not exotic
//! input being handled sloppily; it is ordinary Scripture losing words.
//!
//! It only bites when text follows the pipe on the same line, which is why it
//! survived a 200-file corpus: most such texts put the danda at end of line,
//! where there is nothing after it to lose.
//!
//! # The repair
//!
//! Fixing the lexer means forking the parser, which [ADR-001](../../../docs/adr/001-parser.md)
//! keeps as a control rather than a habit. The facade can do it instead,
//! because `ParsedDocument::tokens()` is public: the attribute tokens are
//! still in the token stream with their source offsets, even though the tree
//! forgot them.
//!
//! So: any attribute token that does **not** fall inside a node that can
//! legitimately carry attributes was literal text, and is put back — pipe
//! included, because the pipe is the punctuation.

use usfm3::lexer::TokenSpan;

use crate::node::{Node, NodeKind};
use crate::span::ByteSpan;

/// Node kinds that may legitimately carry attributes.
///
/// A `|` inside one of these was correctly consumed. A `|` anywhere else was
/// punctuation the lexer misread.
fn consumes_attributes(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Char
            | NodeKind::Figure
            | NodeKind::Milestone
            | NodeKind::Reference
            | NodeKind::Note
    )
}

/// Put back every attribute token the tree dropped.
pub(crate) fn restore(content: &mut [Node], tokens: &[TokenSpan]) {
    let mut candidates: Vec<ByteSpan> = tokens
        .iter()
        .filter(|t| t.kind == "attributes")
        .map(|t| ByteSpan::new(t.start, t.end))
        .collect();
    if candidates.is_empty() {
        return;
    }

    // Anything inside a marker that takes attributes was consumed properly.
    let mut consuming = Vec::new();
    for node in content.iter() {
        collect_consuming(node, &mut consuming);
    }
    candidates.retain(|c| {
        !consuming
            .iter()
            .any(|s| s.start <= c.start && c.end <= s.end)
    });
    if candidates.is_empty() {
        return;
    }

    for node in content.iter_mut() {
        insert_into(node, &candidates, tokens);
    }
}

fn collect_consuming(node: &Node, out: &mut Vec<ByteSpan>) {
    if consumes_attributes(node.kind) {
        if let Some(span) = &node.span {
            out.push(span.clone());
        }
    }
    for child in &node.children {
        collect_consuming(child, out);
    }
}

fn insert_into(node: &mut Node, candidates: &[ByteSpan], tokens: &[TokenSpan]) {
    for child in &mut node.children {
        insert_into(child, candidates, tokens);
    }

    let Some(span) = node.span.clone() else {
        return;
    };
    // Only the node that actually encloses the token, and only one that holds
    // content: putting the text on a chapter marker would be worse than
    // losing it.
    if !matches!(node.kind, NodeKind::Para | NodeKind::TableCell) {
        return;
    }

    for candidate in candidates {
        if candidate.start < span.start || candidate.end > span.end {
            continue;
        }
        // A deeper node already claimed it.
        if node.children.iter().any(|c| {
            encloses(c, candidate) && matches!(c.kind, NodeKind::Para | NodeKind::TableCell)
        }) {
            continue;
        }
        let Some(text) = tokens
            .iter()
            .find(|t| t.start == candidate.start && t.end == candidate.end)
            .map(|t| t.text.clone())
        else {
            continue;
        };

        let mut restored = Node::new(NodeKind::Text);
        restored.text = Some(text);

        // Before the first child that starts after this token; otherwise at
        // the end. Text leaves carry no span, so position is decided by the
        // structural children around them — which is enough, because the
        // lexer only ends an attribute run at a marker or a newline.
        let at = node
            .children
            .iter()
            .position(|c| c.span.as_ref().is_some_and(|s| s.start >= candidate.end))
            .unwrap_or(node.children.len());
        node.children.insert(at, restored);
    }
}

fn encloses(node: &Node, span: &ByteSpan) -> bool {
    node.span
        .as_ref()
        .is_some_and(|s| s.start <= span.start && span.end <= s.end)
}

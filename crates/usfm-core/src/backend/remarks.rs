//! Putting back blocks the parser nested inside a paragraph after a `\rem`.
//!
//! # The bug
//!
//! When a `\rem` arrives while a paragraph is open, `usfm3` nests the remark
//! *inside* that paragraph (`cst.rs`, the `Meta` arm). That is deliberate and
//! harmless on its own. The fault is in what comes next. The next block
//! marker — a `\c`, a `\p`, any paragraph — closes the open paragraph context
//! with `close_paragraph`, which pops to the first paragraph-like entry it
//! finds and stops, and the first one it finds is the `\rem`. The paragraph
//! that held it stays open, and the new block becomes its child:
//!
//! ```text
//! \ili1 licence            Para ili1
//! \rem made by a tool        Para rem
//! \c 1                       Chapter 1      ← should be a sibling of ili1
//! \p                       Para p
//! ```
//!
//! A chapter does not stay open, so the marker after it closes the paragraph
//! properly and the damage ends there. A paragraph does stay open, and when
//! it is closed the outer one is exposed again — so a `\p` after the `\rem`
//! nests, and the `\c` after *that* nests too.
//!
//! The validator only looks for chapters and body paragraphs at the top
//! level, so it never sees these. Every body paragraph of the chapter is then
//! reported as coming before the first `\c`, the next chapter as out of
//! sequence, a one-chapter book as having no chapter at all, and mid-book the
//! verse numbers that restart at the unseen chapter as out of order — while a
//! body paragraph that really is before the first chapter goes unreported.
//!
//! # Why it is common
//!
//! A tool that writes a `\rem` saying it produced the file, at the end of the
//! introduction and right before `\c 1`, does it in every book. The case that
//! found it was a Gujarati New Testament exported through Prince: 214 false
//! errors across 27 books, and not one real one among them.
//!
//! # The repair
//!
//! Fixed upstream in the 0.3 rewrite, but that is a different API, and moving
//! to it is ADR-001's deliberate, corpus-gated kind of change, not a bug fix.
//! So the facade repairs 0.2's output instead, as [`super::pipes`] does:
//!
//! - **The tree.** In a top-level paragraph, every chapter, and every
//!   paragraph other than a `\rem` that comes after a `\rem`, is moved out to
//!   the top level — the shape the parser produces when there is no `\rem`.
//!   Text that followed a moved chapter inside the paragraph stays after it,
//!   in a continuation of the same paragraph. The AST and its source map are
//!   split in lockstep, so every span is still the parser's own.
//! - **The diagnostics.** The parser's validation pass is public. Its verdict
//!   on the broken tree is taken out, and its verdict on the repaired tree put
//!   in, so the rules applied are still exactly upstream's — nothing here
//!   re-implements them. Diagnostics from building the tree are untouched.
//!
//! A document with nothing nested this way is not copied at all.

use usfm3::ast::{Document, Node};
use usfm3::diagnostics::Diagnostic;
use usfm3::source_map::{SourceMap, SourceNode, SourceSpans};
use usfm3::validation::validate;

/// The parser's tree with every wrongly nested block moved to the top level.
pub(super) struct Repaired {
    pub(super) ast: Document<'static>,
    pub(super) source_map: SourceMap,
}

/// `None` when nothing needs moving, which is the usual case.
pub(super) fn repair(ast: &Document<'static>, source_map: &SourceMap) -> Option<Repaired> {
    // Without a source map of the same shape there are no spans to move, and
    // a tree whose blocks have spans that disagree with their position is
    // worse than the tree the parser gave.
    if source_map.content.len() != ast.content.len() {
        return None;
    }
    if !ast.content.iter().any(|node| hoisted(node).contains(&true)) {
        return None;
    }

    let mut content = Vec::with_capacity(ast.content.len() + 2);
    let mut map = Vec::with_capacity(ast.content.len() + 2);
    for (node, source) in ast.content.iter().zip(&source_map.content) {
        match split(node, source) {
            Some(parts) => {
                for (node, source) in parts {
                    content.push(node);
                    map.push(source);
                }
            }
            None => {
                content.push(node.clone());
                map.push(source.clone());
            }
        }
    }

    Some(Repaired {
        ast: Document { content },
        source_map: SourceMap { content: map },
    })
}

/// Replace the validator's findings on the broken tree with its findings on
/// the repaired one.
pub(super) fn revalidate(
    diagnostics: &mut Vec<Diagnostic>,
    ast: &Document<'static>,
    source_map: &SourceMap,
    repaired: &Repaired,
) {
    for stale in validate(ast, source_map).into_inner() {
        if let Some(at) = diagnostics.iter().position(|d| *d == stale) {
            diagnostics.remove(at);
        }
    }
    diagnostics.extend(validate(&repaired.ast, &repaired.source_map).into_inner());
    diagnostics.sort_by_key(|d| d.span.start);
}

fn is_rem(node: &Node<'_>) -> bool {
    matches!(node, Node::Para { marker, .. } if marker.as_str() == "rem")
}

/// For each child of a top-level paragraph, whether it belongs at the top
/// level instead. Empty for anything that is not a paragraph.
fn hoisted(node: &Node<'_>) -> Vec<bool> {
    let Node::Para { content, .. } = node else {
        return Vec::new();
    };
    let mut after_rem = false;
    content
        .iter()
        .map(|child| {
            let out = match child {
                Node::Chapter(_) => true,
                Node::Para { .. } => after_rem && !is_rem(child),
                _ => false,
            };
            after_rem |= is_rem(child);
            out
        })
        .collect()
}

/// A paragraph holding blocks that belong beside it, as the paragraph up to
/// the first of them, each of them in turn, and any text that followed a
/// chapter.
fn split(node: &Node<'static>, source: &SourceNode) -> Option<Vec<(Node<'static>, SourceNode)>> {
    let Node::Para { marker, content } = node else {
        return None;
    };
    let out = hoisted(node);
    if !out.contains(&true) || source.children.len() != content.len() {
        return None;
    }

    let whole = source.spans.as_ref().map(|spans| spans.node.clone());
    let mut parts = Vec::new();
    let mut run: Vec<(Node<'static>, SourceNode)> = Vec::new();
    let mut run_start = whole.as_ref().map(|span| span.start);
    let mut first = true;

    let close_run = |run: &mut Vec<(Node<'static>, SourceNode)>,
                     parts: &mut Vec<(Node<'static>, SourceNode)>,
                     start: Option<usize>,
                     end: Option<usize>,
                     first: bool| {
        // The opening piece is the paragraph as written, so it is kept even if
        // the block came straight after its marker. A continuation that is
        // only the line break after `\c 1` is not a paragraph.
        if !first && run.iter().all(|(node, _)| is_blank(node)) {
            run.clear();
            return;
        }
        let (nodes, children): (Vec<_>, Vec<_>) = std::mem::take(run).into_iter().unzip();
        let spans = match (start, end) {
            (Some(start), Some(end)) if first => source.spans.clone().map(|mut spans| {
                spans.node = start..end.max(start);
                spans
            }),
            (Some(start), Some(end)) => Some(SourceSpans::node(start..end.max(start))),
            _ => None,
        };
        parts.push((
            Node::Para {
                marker: *marker,
                content: nodes,
            },
            SourceNode {
                spans,
                children,
                anchor_cst: if first { source.anchor_cst } else { None },
            },
        ));
    };

    for ((child, child_source), out) in content.iter().zip(&source.children).zip(out) {
        if !out {
            run.push((child.clone(), child_source.clone()));
            continue;
        }
        let spans = child_source.spans.as_ref();
        close_run(
            &mut run,
            &mut parts,
            run_start,
            spans.map(|s| s.node.start),
            first,
        );
        parts.push((child.clone(), child_source.clone()));
        run_start = spans.map(|s| {
            let number_end = s.number.as_ref().map_or(s.node.end, |n| n.end);
            s.node.end.max(number_end)
        });
        first = false;
    }
    close_run(
        &mut run,
        &mut parts,
        run_start,
        whole.map(|span| span.end),
        first,
    );

    Some(parts)
}

fn is_blank(node: &Node<'_>) -> bool {
    matches!(node, Node::Text(text) if text.trim().is_empty())
}

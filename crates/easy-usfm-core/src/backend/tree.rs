//! The parser's syntax tree to ours, reconciled with its parallel source map.
//!
//! ADR-001 records this as the main API friction: the parser's nodes do not
//! carry spans, and source locations live in a second tree of the same shape.
//! The two are walked in lockstep and paired by position — the same pairing
//! the parser's own USJ serializer uses, so we agree with it by construction
//! rather than by coincidence, which is what makes the differential oracle
//! (P0.10) meaningful.

use usfm3::ast::{Attribute as UpstreamAttribute, Document, Node as UpstreamNode};
use usfm3::markers::MarkerName;
use usfm3::source_map::{SourceMap, SourceNode};

use crate::{Attribute, Marker, Node, NodeKind};

pub(super) fn convert_document(ast: &Document<'_>, source_map: &SourceMap) -> Vec<Node> {
    convert_nodes(&ast.content, Some(&source_map.content))
}

fn convert_nodes(nodes: &[UpstreamNode<'_>], source: Option<&[SourceNode]>) -> Vec<Node> {
    // The parser treats a shape disagreement between the two trees as a hard
    // error. We drop spans for the level instead: a tree without spans is
    // still editable, still saveable, and still shows the user their text,
    // whereas failing the parse would take the whole document down over a
    // defect in location metadata. ADR-003 — the source is authoritative, and
    // everything derived from it is disposable.
    let source = source.filter(|source| source.len() == nodes.len());

    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| convert_node(node, source.and_then(|source| source.get(index))))
        .collect()
}

fn convert_node(node: &UpstreamNode<'_>, source: Option<&SourceNode>) -> Node {
    let children = source.map(|source| source.children.as_slice());

    let mut converted = match node {
        UpstreamNode::Text(text) => {
            let mut converted = Node::new(NodeKind::Text);
            converted.text = Some(text.to_string());
            converted
        }

        UpstreamNode::OptBreak => Node::new(NodeKind::OptBreak),

        UpstreamNode::Book {
            marker,
            code,
            content,
        } => {
            let mut converted = marked(NodeKind::Book, marker);
            converted.attributes.push(Attribute::new("code", &**code));
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Chapter(data) => {
            let mut converted = marked(NodeKind::Chapter, &data.marker);
            converted
                .attributes
                .push(Attribute::new("number", &*data.number));
            push_optional(&mut converted, "sid", data.sid.as_deref());
            push_optional(&mut converted, "altnumber", data.altnumber.as_deref());
            push_optional(&mut converted, "pubnumber", data.pubnumber.as_deref());
            converted
        }

        UpstreamNode::Verse(data) => {
            let mut converted = marked(NodeKind::Verse, &data.marker);
            converted
                .attributes
                .push(Attribute::new("number", &*data.number));
            push_optional(&mut converted, "sid", data.sid.as_deref());
            push_optional(&mut converted, "altnumber", data.altnumber.as_deref());
            push_optional(&mut converted, "pubnumber", data.pubnumber.as_deref());
            converted
        }

        UpstreamNode::Para { marker, content } => {
            let mut converted = marked(NodeKind::Para, marker);
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Char(data) => {
            let mut converted = marked(NodeKind::Char, &data.marker);
            converted.attributes = convert_attributes(&data.attributes);
            converted.children = convert_nodes(&data.content, children);
            converted
        }

        UpstreamNode::Note {
            marker,
            caller,
            category,
            content,
        } => {
            let mut converted = marked(NodeKind::Note, marker);
            converted
                .attributes
                .push(Attribute::new("caller", &**caller));
            push_optional(&mut converted, "category", category.as_deref());
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Milestone { marker, attributes } => {
            let mut converted = marked(NodeKind::Milestone, marker);
            converted.attributes = convert_attributes(attributes);
            converted
        }

        UpstreamNode::Figure {
            marker,
            content,
            attributes,
        } => {
            let mut converted = marked(NodeKind::Figure, marker);
            converted.attributes = convert_attributes(attributes);
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Sidebar {
            marker,
            category,
            content,
        } => {
            let mut converted = marked(NodeKind::Sidebar, marker);
            push_optional(&mut converted, "category", category.as_deref());
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Periph {
            alt,
            content,
            attributes,
        } => {
            let mut converted = Node::new(NodeKind::Periph);
            converted.attributes = convert_attributes(attributes);
            push_optional(&mut converted, "alt", alt.as_deref());
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Table { content } => {
            let mut converted = Node::new(NodeKind::Table);
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::TableRow { marker, content } => {
            let mut converted = marked(NodeKind::TableRow, marker);
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::TableCell {
            marker,
            align,
            content,
        } => {
            let mut converted = marked(NodeKind::TableCell, marker);
            converted.attributes.push(Attribute::new("align", &**align));
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Ref {
            content,
            attributes,
        } => {
            let mut converted = Node::new(NodeKind::Reference);
            converted.attributes = convert_attributes(attributes);
            converted.children = convert_nodes(content, children);
            converted
        }

        UpstreamNode::Unknown { marker, content } => {
            let mut converted = marked(NodeKind::Unknown, marker);
            converted.children = convert_nodes(content, children);
            converted
        }
    };

    // Text and optional breaks are recorded in the source map as bare leaves —
    // no span, no anchor — so both stay `None` here. See `Node::span`.
    converted.span = source
        .and_then(|source| source.spans.as_ref())
        .map(|spans| spans.node.clone().into());
    converted.anchor_cst = source.and_then(|source| source.anchor_cst);

    converted
}

fn marked(kind: NodeKind, marker: &MarkerName) -> Node {
    let mut node = Node::new(kind);
    node.marker = Some(Marker::new(marker.as_str()));
    node
}

fn push_optional(node: &mut Node, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        node.attributes.push(Attribute::new(key, value));
    }
}

fn convert_attributes(attributes: &[UpstreamAttribute<'_>]) -> Vec<Attribute> {
    attributes
        .iter()
        .map(|attribute| Attribute::new(&*attribute.key, &*attribute.value))
        .collect()
}

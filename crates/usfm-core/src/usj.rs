//! USJ output — the form the differential oracle compares.
//!
//! ADR-004 adopts the USJ content model, and the practical payoff is here:
//! two independent mature implementations emit USJ natively, so all three can
//! be diffed **structurally** rather than by comparing rendered output or
//! eyeballing behaviour (ARCHITECTURE §12.1). A bespoke model would need a
//! translation layer for the comparison, which would itself need testing,
//! which defeats the purpose.
//!
//! This is also what makes USX and USJ export a day's work whenever they are
//! wanted — not built now, but excluded at no cost later.
//!
//! Spans are deliberately absent. USJ has no notion of them, and the oracle is
//! comparing what the parsers *understood*, not where they found it. Source
//! locations are checked separately, by the offset properties (P0.3) and the
//! equivalence suite (P0.5).

use serde_json::{Map, Value};

use crate::{Node, NodeKind};

/// The USJ version this shape conforms to.
const USJ_VERSION: &str = "3.1";

/// Renders a document tree as USJ.
pub fn to_usj(content: &[Node]) -> Value {
    let mut root = Map::new();
    root.insert("type".into(), Value::String("USJ".into()));
    root.insert("version".into(), Value::String(USJ_VERSION.into()));
    root.insert("content".into(), Value::Array(nodes(content)));
    Value::Object(root)
}

fn nodes(list: &[Node]) -> Vec<Value> {
    list.iter().map(node).collect()
}

fn node(node: &Node) -> Value {
    // Text is a bare string in USJ, not an object. The model treats prose as
    // the default rather than as a special kind of element.
    if node.kind == NodeKind::Text {
        return Value::String(node.text.clone().unwrap_or_default());
    }

    let mut map = Map::new();
    map.insert("type".into(), Value::String(node.kind.as_str().into()));

    if let Some(marker) = &node.marker {
        map.insert("marker".into(), Value::String(marker.as_str().into()));
    }

    // Our tree keeps a chapter's `number` and a figure's `src` in one place,
    // as USJ itself does; emitting is putting them back as properties.
    for attribute in &node.attributes {
        map.insert(
            attribute.key.clone(),
            Value::String(attribute.value.clone()),
        );
    }

    // Omitted when empty rather than emitted as `[]`, matching how USJ is
    // written in practice.
    if !node.children.is_empty() {
        map.insert("content".into(), Value::Array(nodes(&node.children)));
    }

    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    fn usj_of(source: &str) -> Value {
        let document = Document::parse(source.to_string());
        to_usj(document.content())
    }

    #[test]
    fn the_root_declares_the_model() {
        let usj = usj_of("\\id GEN\n");
        assert_eq!(usj["type"], "USJ");
        assert_eq!(usj["version"], USJ_VERSION);
        assert!(usj["content"].is_array());
    }

    #[test]
    fn text_is_a_bare_string() {
        let usj = usj_of("\\id GEN Genesis\n");
        let book = &usj["content"][0];
        assert_eq!(book["type"], "book");
        assert_eq!(book["code"], "GEN");
        assert_eq!(book["content"][0], "Genesis");
    }

    #[test]
    fn chapter_and_verse_properties_are_top_level() {
        let usj = usj_of("\\id GEN\n\\c 1\n\\p\n\\v 1 text\n");
        let chapter = usj["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["type"] == "chapter")
            .expect("a chapter");

        assert_eq!(chapter["number"], "1");
        assert_eq!(chapter["marker"], "c");
    }

    #[test]
    fn a_node_with_no_children_omits_content() {
        let usj = usj_of("\\id GEN\n\\c 1\n");
        let chapter = usj["content"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["type"] == "chapter")
            .expect("a chapter");

        assert!(chapter.get("content").is_none());
    }

    #[test]
    fn attributes_become_properties() {
        let usj = usj_of("\\id GEN\n\\c 1\n\\p\n\\v 1 \\w grace|lemma=\"grace\"\\w*\n");
        let json = usj.to_string();
        assert!(json.contains("\"lemma\":\"grace\""), "{json}");
    }
}

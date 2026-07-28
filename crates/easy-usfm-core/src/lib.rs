//! The USFM engine: parsing, the document model, diagnostics, and source
//! locations.
//!
//! ```
//! use easy_usfm_core::{Document, NodeKind};
//!
//! let document = Document::parse("\\id GEN\n\\c 1\n\\p\n\\v 1 In the beginning\n");
//! let kinds: Vec<_> = document.descendants().map(|node| node.kind).collect();
//!
//! assert!(kinds.contains(&NodeKind::Chapter));
//! assert!(kinds.contains(&NodeKind::Verse));
//! ```
//!
//! # What this crate is for
//!
//! It builds standalone — no Tauri, no filesystem, no interface — and compiles
//! to `wasm32-unknown-unknown`, because there is one engine on every target
//! and it runs in a worker (ADR-002). Nothing here reads a file or draws
//! anything.
//!
//! # Two boundaries that are load-bearing
//!
//! **Which parser sits underneath is not public.** `usfm3` is pinned exactly
//! and confined to `src/backend/`; no type of its appears in this crate's API,
//! and `tests/facade_boundary.rs` fails the build if it is named anywhere
//! else. That containment is what makes ADR-001's risk controls real rather
//! than aspirational.
//!
//! **Byte offsets do not leave.** [`ByteSpan`] has no `Serialize` impl, so a
//! byte offset cannot reach JavaScript — where it would index a UTF-16 string
//! and land in the wrong place, silently, and only for non-ASCII text.
//! Converting at the boundary is `Utf16Mapper`'s job, which arrives with P0.3.
//! UNICODE §1.

mod backend;
mod diagnostic;
mod document;
mod node;
mod span;

pub use diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use document::Document;
pub use node::{Attribute, Marker, Node, NodeKind};
pub use span::ByteSpan;

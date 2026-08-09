//! The USFM engine: parsing, the document model, diagnostics, and source
//! locations.
//!
//! ```
//! use usfm_core::{Document, NodeKind};
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
//! [`Char16`] does have one, and only [`Utf16Mapper`] can produce it, so
//! conversion is the single narrow path out. Use [`Document::to_char16`],
//! which cannot be handed a source that disagrees with its index. UNICODE §1.

mod backend;
mod char16;
mod completion;
mod diagnostic;
mod document;
mod fidelity;
pub mod grapheme;
pub mod invariants;
pub mod joiners;
mod lines;
pub mod markers;
mod node;
mod normalize;
pub(crate) mod reference;
mod session;
mod severity;
mod span;
mod text_span;
mod usj;
mod verse;
mod version;

pub use char16::{Char16, Char16Range, Utf16Mapper};
pub use completion::{completions, frequencies, Completion, Context as CompletionContext};
pub use diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use document::Document;
pub use fidelity::{DecodeError, Eol, FileFidelity, LineEndings, Loaded};
pub use lines::{LineCol, LineIndex};
pub use node::{Attribute, Marker, Node, NodeKind};
pub use normalize::NormalizedIndex;
pub use reference::{decimal_value, parse_digits, Reference, Resolution};
pub use session::{Applied, Chunk, Edit, EditError, Session};
pub use severity::DiagnosticConfig;
pub use span::ByteSpan;
pub use text_span::{locate as locate_text, Cursor as TextCursor};
pub use usj::to_usj;
pub use verse::{VerseEntry, VerseId, VerseIndex};
pub use version::Version;

mod checksum;
mod token;

/// The starting point for a new document (PRODUCT 6.3).
///
/// Here rather than in either shell because there are two of them, and a
/// second copy is a second answer to "what does New give me" that depends on
/// whether you opened the desktop application or the web one.
///
/// The book code is deliberately a placeholder: PRODUCT 6.3 puts the cursor
/// after it and expects the book-code diagnostic to fire immediately, which
/// is the intended teaching moment.
pub const NEW_DOCUMENT: &str =
    "\\id XXA\n\\h \n\\toc1 \n\\toc2 \n\\toc3 \n\\mt1 \n\\c 1\n\\p\n\\v 1 \n";

pub use checksum::checksum;
pub use token::{Token, TokenKind};

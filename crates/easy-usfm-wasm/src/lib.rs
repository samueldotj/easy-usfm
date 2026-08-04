//! The engine's surface to JavaScript.
//!
//! ARCHITECTURE §2: **one engine, compiled once.** The same artifact runs in a
//! worker on the desktop and in the browser; there is no native parsing path
//! and no second implementation to keep in step. ADR-002 has the reasoning.
//!
//! # What crosses, and what does not
//!
//! **Not the document.** ARCHITECTURE §9: the text is mirrored, not shipped.
//! Sending a 2 MB string on every debounce would mean a transcode and an
//! allocation per keystroke, so this side holds a [`Session`] and receives
//! edits. That is why the surface is a handle with methods rather than a
//! `parse(text)` function — the shape is the protocol.
//!
//! **Not byte offsets.** Everything here reports Char16, because a byte offset
//! that reached JavaScript would index a UTF-16 string and land in the wrong
//! place — silently, and only for non-ASCII text (UNICODE §1). The conversion
//! happens at this boundary and nowhere else.

use easy_usfm_core::{
    ByteSpan, Char16, Char16Range, Session as CoreSession, Severity, Utf16Mapper,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

/// Sets up panic reporting. Idempotent; the worker calls it once on load.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// The engine's version, which is also the round trip that proves the worker
/// loaded a module rather than a stub.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ------------------------------------------------------------- payloads ---

/// A diagnostic, in the coordinates the interface works in.
#[derive(Debug, Serialize)]
pub struct WireDiagnostic {
    pub code: &'static str,
    pub severity: &'static str,
    /// UTF-16 code units, which is what CodeMirror and DOM ranges count in.
    pub start: u32,
    pub end: u32,
    pub message: String,
}

/// One chapter's worth of document.
///
/// The unit the preview renders and re-renders (ARCHITECTURE §10): a keyed
/// each block over these means only the chunk whose `rev` changed is rebuilt.
#[derive(Debug, Serialize)]
pub struct WireChunk {
    /// `None` for the header chunk — everything before `\c 1`.
    pub number: Option<u32>,
    pub start: u32,
    pub end: u32,
    pub rev: u64,
}

/// What the worker answers a parse request with.
#[derive(Debug, Serialize)]
pub struct WireResult {
    pub rev: u64,
    pub chunks: Vec<WireChunk>,
    pub diagnostics: Vec<WireDiagnostic>,
    /// The document's length in UTF-16 units, so the caller can check its
    /// mirror is the same length before trusting any offset in here.
    pub len: u32,
}

/// Why an edit was refused.
#[derive(Debug, Serialize)]
struct WireError {
    error: String,
    /// The caller must resend the whole document; its mirror is not what this
    /// side holds.
    resync: bool,
}

// -------------------------------------------------------------- session ---

/// An open document, held on this side of the boundary.
#[wasm_bindgen]
pub struct Session {
    inner: CoreSession,
    mapper: Utf16Mapper,
}

#[wasm_bindgen]
impl Session {
    /// Opens a document. The one time the full text crosses.
    #[wasm_bindgen(constructor)]
    pub fn new(source: String) -> Self {
        let inner = CoreSession::new(source);
        let mapper = Utf16Mapper::new(inner.source());
        Self { inner, mapper }
    }

    /// The document as this side holds it.
    ///
    /// For resynchronisation and for the checksum comparison that detects
    /// drift (ARCHITECTURE §9). Not for ordinary use — it is the expensive
    /// path the delta protocol exists to avoid.
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.inner.source().to_string()
    }

    /// Length in UTF-16 units, which is what `String.length` reports in
    /// JavaScript. The cheap half of desync detection.
    ///
    /// Named `length` on the JavaScript side to match what it is being
    /// compared against, and not `len` on this side because a `Session` is not
    /// a collection — clippy is right to ask for `is_empty` beside a `len`,
    /// and an empty session is not a thing.
    #[wasm_bindgen(getter, js_name = length)]
    pub fn char16_length(&self) -> u32 {
        self.mapper.len_char16().get()
    }

    #[wasm_bindgen(getter)]
    pub fn rev(&self) -> u64 {
        self.inner.rev()
    }

    /// Applies one edit, in UTF-16 offsets against the document as this side
    /// currently holds it.
    ///
    /// Returns the parse result. Refusing rather than approximating is
    /// deliberate: an edit applied inexactly desynchronises the mirror, and a
    /// desynchronised mirror corrupts every offset in the interface.
    pub fn edit(&mut self, from: u32, to: u32, insert: &str) -> Result<JsValue, JsValue> {
        let source = self.inner.source().to_string();

        // The editor already counts in UTF-16, so these arrive in the right
        // space. What they still have to survive is the conversion to bytes,
        // which refuses an offset that falls between the halves of a surrogate
        // pair -- a position naming no character.
        let Some(start) = self.mapper.to_byte(&source, Char16::from_editor(from)) else {
            return Err(refuse("edit start is not on a character boundary"));
        };
        let Some(end) = self.mapper.to_byte(&source, Char16::from_editor(to)) else {
            return Err(refuse("edit end is not on a character boundary"));
        };

        self.inner
            .edit(ByteSpan::new(start, end), insert)
            .map_err(|error| refuse(&error.to_string()))?;

        // The mapper indexes the text, so it is rebuilt whenever the text
        // changes. Doing this incrementally is the obvious optimisation and
        // is not taken until it is measured -- P0.4 showed the parse itself
        // costs 65 microseconds, so guessing where the time goes is not worth
        // the correctness risk.
        self.mapper = Utf16Mapper::new(self.inner.source());
        Ok(self.result())
    }

    /// Replaces the document wholesale. Used on open, on a detected desync,
    /// and after an external reload.
    pub fn resync(&mut self, source: String) -> JsValue {
        self.inner = CoreSession::new(source);
        self.mapper = Utf16Mapper::new(self.inner.source());
        self.result()
    }

    /// The current parse, without changing anything.
    pub fn snapshot(&self) -> JsValue {
        self.result()
    }

    fn result(&self) -> JsValue {
        let source = self.inner.source();

        let chunks = self
            .inner
            .chunks()
            .iter()
            .map(|chunk| {
                let range = self.to_char16(source, &chunk.range());
                WireChunk {
                    number: chunk.number(),
                    start: range.start.get(),
                    end: range.end.get(),
                    rev: chunk.rev(),
                }
            })
            .collect();

        let diagnostics = self
            .inner
            .diagnostics()
            .into_iter()
            .map(|diagnostic| {
                let range = self.to_char16(source, &diagnostic.span);
                WireDiagnostic {
                    code: diagnostic.code.as_str(),
                    severity: match diagnostic.severity {
                        Severity::Error => "error",
                        Severity::Warning => "warning",
                        Severity::Information => "information",
                    },
                    start: range.start.get(),
                    end: range.end.get(),
                    message: diagnostic.message,
                }
            })
            .collect();

        let result = WireResult {
            rev: self.inner.rev(),
            chunks,
            diagnostics,
            len: self.mapper.len_char16().get(),
        };

        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Byte offsets to UTF-16, at the one boundary where that conversion is
    /// allowed to happen.
    fn to_char16(&self, source: &str, span: &ByteSpan) -> Char16Range {
        self.mapper
            .to_char16_range(source, span)
            .unwrap_or(Char16Range {
                start: self.mapper.len_char16(),
                end: self.mapper.len_char16(),
            })
    }
}

fn refuse(message: &str) -> JsValue {
    serde_wasm_bindgen::to_value(&WireError {
        error: message.to_string(),
        resync: true,
    })
    .unwrap_or(JsValue::NULL)
}

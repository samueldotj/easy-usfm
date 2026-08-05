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
    ByteSpan, Char16, Char16Range, Resolution, Session as CoreSession, Severity, TokenKind,
    Utf16Mapper, Version,
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
    /// 1-based, for the panel to say where. Computed here because the mapper
    /// is line-indexed already; the alternative is the interface walking the
    /// document on every keystroke to recover it.
    pub line: u32,
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

/// One highlighted run.
///
/// Carries a class name rather than a colour: appearance lives in a stylesheet
/// Vite extracts at build time, never in an injected theme (SECURITY §5).
#[derive(Debug, Serialize)]
pub struct WireToken {
    pub class: &'static str,
    pub start: u32,
    pub end: u32,
}

/// The document's USFM version, as the status bar needs to say it.
///
/// `declared` is separate from `effective` because "says nothing" is not the
/// same as "says 3.0" — most files in circulation carry no `\usfm` line and
/// are valid (PRODUCT §4), and reporting only the effective version would make
/// the status bar claim a declaration the file never made.
#[derive(Debug, Serialize)]
pub struct WireVersion {
    /// What the file declares, or `None`.
    pub declared: Option<String>,
    /// What diagnostics are judged against.
    pub effective: String,
    /// Whether that came from the user rather than the file.
    pub overridden: bool,
    /// What a file declaring nothing is taken to be.
    ///
    /// Sent rather than assumed on the other side. The interface has to name
    /// this number while an override is in force -- to say what clearing the
    /// override would go back to -- and it cannot derive it from `effective`
    /// at that moment. Hardcoding it there would put the same constant in two
    /// languages, which is how the two stop agreeing.
    pub assumed: String,
}

/// What came of looking up a reference.
///
/// The failure carries a sentence rather than a code, because every one of
/// them means something different to the person who typed it and the engine is
/// the only side that knows which happened. "Not found" would send someone
/// looking for the wrong problem — most often for a verse that is in a
/// different file entirely.
#[derive(Debug, Serialize)]
pub struct WireResolution {
    /// Char16, when it resolved.
    pub start: Option<u32>,
    pub end: Option<u32>,
    /// What to say when it did not.
    pub message: Option<String>,
}

/// What the worker answers a parse request with.
#[derive(Debug, Serialize)]
pub struct WireResult {
    pub rev: u64,
    pub chunks: Vec<WireChunk>,
    pub diagnostics: Vec<WireDiagnostic>,
    pub version: WireVersion,
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

    /// The mirror's checksum, for comparison against the editor's.
    ///
    /// ARCHITECTURE §9: silent drift corrupts every offset in the interface,
    /// and nothing about the display says so. This is the only way the two
    /// sides find out they have stopped agreeing.
    #[wasm_bindgen(getter)]
    pub fn checksum(&self) -> u32 {
        easy_usfm_core::checksum(self.inner.source())
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

    /// Overrides the document's USFM version, or returns to what the file says.
    ///
    /// `null` clears the override. An unparseable string is treated as
    /// clearing it too, rather than refused: this arrives from a control whose
    /// values this side does not define, and a desync — which is what a
    /// refusal becomes — is a wildly disproportionate answer to a bad dropdown
    /// value.
    ///
    /// Reparses nothing. The severity that depends on this is derived at query
    /// time, so the answer changes and the parse does not (ARCHITECTURE §8.1).
    #[wasm_bindgen(js_name = overrideVersion)]
    pub fn override_version(&mut self, version: Option<String>) -> JsValue {
        self.inner
            .override_version(version.as_deref().and_then(Version::parse));
        self.result()
    }

    /// The current parse, without changing anything.
    pub fn snapshot(&self) -> JsValue {
        self.result()
    }

    /// Tokens covering a Char16 range, for highlighting.
    ///
    /// Range-scoped because the caller is a viewport. Lexing 2 MB to paint
    /// forty visible lines would put the expensive work on the one path that
    /// has to keep up with typing (ARCHITECTURE §8.1).
    pub fn tokens(&self, from: u32, to: u32) -> JsValue {
        let source = self.inner.source();

        let start = self
            .mapper
            .to_byte(source, Char16::from_editor(from))
            .unwrap_or(0);
        let end = self
            .mapper
            .to_byte(source, Char16::from_editor(to))
            .unwrap_or(source.len());

        let tokens: Vec<WireToken> = self
            .inner
            .tokens(&ByteSpan::new(start, end))
            .into_iter()
            // Text is the default appearance, so sending it would be a
            // decoration per word for no visible effect.
            .filter(|token| token.kind != TokenKind::Text)
            .map(|token| {
                let range = self.to_char16(source, &token.span);
                WireToken {
                    class: token.kind.css_class(),
                    start: range.start.get(),
                    end: range.end.get(),
                }
            })
            .collect();

        to_js(&tokens)
    }

    /// Go to Reference (PRODUCT §6.2).
    pub fn resolve(&self, text: &str) -> JsValue {
        let source = self.inner.source();

        let answer = match self.inner.resolve(text) {
            Resolution::Found(span) => {
                let range = self.to_char16(source, &span);
                WireResolution {
                    start: Some(range.start.get()),
                    end: Some(range.end.get()),
                    message: None,
                }
            }
            Resolution::Unparseable => {
                refusal("Type a reference like GEN 1:1, 1:1, or 3.".to_string())
            }
            Resolution::WrongBook { document, asked } => refusal(match document {
                Some(document) => {
                    format!("This document is {document}, not {asked}. Open {asked} to go there.")
                }
                None => format!("This document does not say it is {asked}."),
            }),
            Resolution::NoSuchChapter(chapter) => {
                refusal(format!("This document has no chapter {chapter}."))
            }
            Resolution::NoSuchVerse { chapter, verse } => {
                refusal(format!("Chapter {chapter} has no verse {verse}."))
            }
        };

        to_js(&answer)
    }

    /// How a Char16 offset reads as a reference, for the status bar.
    ///
    /// `null` before the first verse, where there is nothing to report — a
    /// header is not at any reference, and inventing one would be a lie the
    /// status bar tells continuously.
    #[wasm_bindgen(js_name = referenceAt)]
    pub fn reference_at(&self, at: u32) -> Option<String> {
        let source = self.inner.source();
        let byte = self
            .mapper
            .to_byte(source, Char16::from_editor(at))
            .unwrap_or(source.len());
        self.inner.reference_at(byte)
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
                    line: self.mapper.line(source, diagnostic.span.start).unwrap_or(1),
                    message: diagnostic.message,
                }
            })
            .collect();

        let result = WireResult {
            rev: self.inner.rev(),
            chunks,
            diagnostics,
            version: WireVersion {
                declared: self.inner.detected_version().map(|v| v.to_string()),
                effective: self.inner.document_version().to_string(),
                overridden: self.inner.version_is_overridden(),
                assumed: Version::ASSUMED.to_string(),
            },
            len: self.mapper.len_char16().get(),
        };

        to_js(&result)
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

/// Serialises a payload for JavaScript.
///
/// The one thing configured here is that `None` crosses as `null` rather than
/// `undefined`, which is serde-wasm-bindgen's default. The difference is
/// invisible until a caller writes the obvious `x === null` check: that is
/// false for `undefined`, so the *failure* branch is skipped and the success
/// code runs with nothing in it. Go to Reference did exactly that -- every
/// unresolvable reference threw inside the editor instead of showing its
/// message.
fn to_js<T: Serialize>(value: &T) -> JsValue {
    const SERIALIZER: serde_wasm_bindgen::Serializer =
        serde_wasm_bindgen::Serializer::new().serialize_missing_as_null(true);
    value.serialize(&SERIALIZER).unwrap_or(JsValue::NULL)
}

fn refusal(message: String) -> WireResolution {
    WireResolution {
        start: None,
        end: None,
        message: Some(message),
    }
}

fn refuse(message: &str) -> JsValue {
    to_js(&WireError {
        error: message.to_string(),
        resync: true,
    })
}

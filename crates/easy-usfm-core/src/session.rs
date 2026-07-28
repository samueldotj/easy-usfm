//! The incremental session: reparse a chapter, not a book.
//!
//! ARCHITECTURE §8.2. Nothing may be O(document) on a keystroke, and two
//! properties of USFM make that reachable. Structural markers are line-initial,
//! so lexing is line-local. And **`\c` at line start is a hard synchronization
//! point** — no construct legally spans a chapter boundary — so the document
//! partitions there into independently parseable chunks, plus a header chunk
//! for everything before `\c 1`.
//!
//! An edit marks its chunk dirty and everything else keeps its parse. The only
//! case where more than one chunk is touched is an edit that creates or
//! destroys a `\c`, which splits or merges neighbours.
//!
//! # Spans are stored chunk-relative
//!
//! The one design decision the rest of this module follows from. A chunk's
//! parse holds offsets relative to the chunk's own start, and they are
//! translated to document coordinates when read.
//!
//! Storing document coordinates instead would be simpler to read and would
//! defeat the entire purpose: inserting one character in chapter 1 shifts
//! every offset in every later chapter, so every cached parse in the document
//! would need rewriting on every keystroke — O(document) work, arrived at by
//! the back door, in the module that exists to avoid it. Chunk-relative
//! storage makes a shift a change to two integers per chunk.

use std::cell::OnceCell;

use crate::backend::Backend;
use crate::severity::{self, DiagnosticConfig};
use crate::{ByteSpan, Diagnostic, DiagnosticCode, Node};

/// One edit, in byte offsets against the document as it was **before** the
/// batch was applied.
///
/// The same coordinates CodeMirror's `iterChanges` reports (`fromA`/`toA`),
/// so a batch arrives needing no translation. Turning editor transactions
/// into these is P2.2's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub range: ByteSpan,
    pub insert: String,
}

impl Edit {
    pub fn new(range: ByteSpan, insert: impl Into<String>) -> Self {
        Self {
            range,
            insert: insert.into(),
        }
    }
}

/// Why an edit was refused.
///
/// Refused rather than best-effort: an edit the session cannot apply exactly
/// would desynchronise the mirrored buffer, and a desynchronised mirror
/// corrupts every offset in the interface (ARCHITECTURE §9). Better to fail
/// the batch and let the frontend resync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The range ran past the end of the document, or was inverted.
    OutOfBounds { range: ByteSpan, len: usize },
    /// An endpoint fell inside a character.
    NotOnCharBoundary { offset: usize },
    /// Edits must be ascending and non-overlapping, as `iterChanges` emits
    /// them.
    Overlapping { previous_end: usize, start: usize },
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfBounds { range, len } => {
                write!(f, "edit {range:?} is outside a document of {len} bytes")
            }
            Self::NotOnCharBoundary { offset } => {
                write!(f, "edit offset {offset} falls inside a character")
            }
            Self::Overlapping {
                previous_end,
                start,
            } => write!(
                f,
                "edit at {start} overlaps the previous, ending {previous_end}"
            ),
        }
    }
}

impl std::error::Error for EditError {}

/// What an applied batch cost.
///
/// Reported so the claim "an edit reparses one chunk" is observable rather
/// than asserted, and so a regression shows up as a number rather than as a
/// gradually slower editor.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Applied {
    pub rev: u64,
    /// Chunks whose parse was discarded.
    pub invalidated: usize,
    /// Their total size — what reparsing will actually cost.
    pub invalidated_bytes: usize,
    /// Chunks that only moved. Their parses were kept.
    pub shifted: usize,
}

/// A chapter's worth of document, parsed independently.
#[derive(Debug)]
pub struct Chunk {
    number: Option<u32>,
    start: usize,
    end: usize,
    rev: u64,
    parsed: OnceCell<ChunkParse>,
}

impl Chunk {
    /// The chapter number, or `None` for the header chunk — everything before
    /// `\c 1`, which is where `\id`, `\h`, and the introduction live.
    pub fn number(&self) -> Option<u32> {
        self.number
    }

    /// Where this chunk sits in the document.
    pub fn range(&self) -> ByteSpan {
        ByteSpan::new(self.start, self.end)
    }

    /// The revision at which this chunk was last invalidated. The preview
    /// re-renders a chunk when this changes and skips it otherwise.
    pub fn rev(&self) -> u64 {
        self.rev
    }

    /// Whether this chunk has been parsed yet.
    pub fn is_parsed(&self) -> bool {
        self.parsed.get().is_some()
    }
}

#[derive(Debug)]
struct ChunkParse {
    /// Chunk-relative. See the module note.
    content: Vec<Node>,
    diagnostics: Vec<Diagnostic>,
}

/// An editable document that reparses incrementally.
pub struct Session {
    source: String,
    chunks: Vec<Chunk>,
    rev: u64,
    config: DiagnosticConfig,
}

impl Session {
    /// Partitions `source` into chunks. Parses nothing.
    pub fn new(source: impl Into<String>) -> Self {
        let source = source.into();
        let config = DiagnosticConfig::for_source(&source);
        Self::with_config(source, config)
    }

    pub fn with_config(source: impl Into<String>, config: DiagnosticConfig) -> Self {
        let source = source.into();
        let chunks = split(&source, 0, 0);
        Self {
            source,
            chunks,
            rev: 0,
            config,
        }
    }

    pub fn config(&self) -> &DiagnosticConfig {
        &self.config
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn rev(&self) -> u64 {
        self.rev
    }

    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// Applies a batch, in the coordinates of the document before the batch.
    ///
    /// The whole batch is validated as it goes; a refusal leaves the session
    /// with the edits applied so far, so a caller that sees `Err` must resync
    /// rather than retry.
    pub fn apply(&mut self, edits: &[Edit]) -> Result<Applied, EditError> {
        let mut previous_end = 0usize;
        for edit in edits {
            if edit.range.start < previous_end {
                return Err(EditError::Overlapping {
                    previous_end,
                    start: edit.range.start,
                });
            }
            previous_end = edit.range.end;
        }

        self.rev += 1;
        let rev = self.rev;

        let mut total = Applied {
            rev,
            ..Applied::default()
        };
        let mut delta = 0isize;

        for edit in edits {
            let range = ByteSpan::new(
                (edit.range.start as isize + delta) as usize,
                (edit.range.end as isize + delta) as usize,
            );
            let one = self.edit_one(&range, &edit.insert, rev)?;

            delta += edit.insert.len() as isize - edit.range.len() as isize;
            total.invalidated += one.invalidated;
            total.invalidated_bytes += one.invalidated_bytes;
            total.shifted += one.shifted;
        }

        Ok(total)
    }

    /// Convenience for the single-edit case.
    pub fn edit(&mut self, range: ByteSpan, insert: &str) -> Result<Applied, EditError> {
        self.apply(&[Edit::new(range, insert)])
    }

    fn edit_one(&mut self, range: &ByteSpan, insert: &str, rev: u64) -> Result<Applied, EditError> {
        if range.start > range.end || range.end > self.source.len() {
            return Err(EditError::OutOfBounds {
                range: range.clone(),
                len: self.source.len(),
            });
        }
        for offset in [range.start, range.end] {
            if !self.source.is_char_boundary(offset) {
                return Err(EditError::NotOnCharBoundary { offset });
            }
        }

        // Which chunks the edit reaches, decided before the text moves.
        let (first, last) = self.affected(range);
        let region_start = self.chunks[first].start;
        let region_end_old = self.chunks[last].end;

        self.source.replace_range(range.start..range.end, insert);
        let delta = insert.len() as isize - range.len() as isize;
        let region_end = (region_end_old as isize + delta) as usize;

        let invalidated_bytes = region_end - region_start;
        let replacement = split(&self.source[region_start..region_end], region_start, rev);

        let shifted = self.chunks.len() - (last + 1);
        self.chunks.splice(first..=last, replacement);

        // Everything after the region only moved. Their parses survive
        // precisely because the offsets inside them are chunk-relative.
        let mut index = self.chunks.len() - shifted;
        while index < self.chunks.len() {
            self.chunks[index].start = (self.chunks[index].start as isize + delta) as usize;
            self.chunks[index].end = (self.chunks[index].end as isize + delta) as usize;
            index += 1;
        }

        let invalidated = self.chunks.len() - shifted - first;

        Ok(Applied {
            rev,
            invalidated,
            invalidated_bytes,
            shifted,
        })
    }

    /// The span of chunks an edit can change the shape of.
    ///
    /// The chunks the edit lands in, widened by one in either direction only
    /// when it could destroy or create a boundary there.
    ///
    /// Widening left matters because an edit inside a chunk's own `\c` line
    /// can destroy the marker, and the chunk then has to merge into its
    /// predecessor — which can only happen if the predecessor is being
    /// rebuilt too. Widening right matters because deleting a trailing
    /// newline joins the final line to the next chunk's `\c`, destroying that
    /// boundary instead. An edit in the middle of a chapter does neither, and
    /// that is the case worth being fast.
    fn affected(&self, range: &ByteSpan) -> (usize, usize) {
        let mut first = self
            .chunks
            .partition_point(|chunk| chunk.start <= range.start)
            .saturating_sub(1);
        let mut last = self
            .chunks
            .partition_point(|chunk| chunk.start <= range.end)
            .saturating_sub(1);

        if range.start < line_end(&self.source, self.chunks[first].start) {
            first = first.saturating_sub(1);
        }
        if last + 1 < self.chunks.len()
            && range.end >= line_start(&self.source, self.chunks[last].end.saturating_sub(1))
        {
            last += 1;
        }

        (first, last)
    }

    /// The book code from `\id`, which chapter chunks need and cannot see.
    ///
    /// Scanned rather than parsed: this is wanted while parsing a chunk, and
    /// parsing the header chunk to get it would make chunk parsing depend on
    /// the order chunks happen to be asked for.
    fn book_code(&self) -> Option<&str> {
        let header = self.chunks.first()?;
        if header.number.is_some() {
            return None; // no header chunk, so no \id anywhere
        }

        self.source[header.start..header.end]
            .lines()
            .find_map(|line| line.strip_prefix("\\id"))
            .and_then(|rest| {
                rest.starts_with([' ', '\t'])
                    .then(|| rest.split_whitespace().next())
                    .flatten()
            })
    }

    fn parse(&self, index: usize) -> &ChunkParse {
        let chunk = &self.chunks[index];
        chunk.parsed.get_or_init(|| {
            let text = &self.source[chunk.start..chunk.end];
            let is_header = chunk.number.is_none();

            // A chapter is parsed inside the minimal context that makes it
            // well-formed, which today means a synthetic `\id`.
            //
            // Not a nicety. `sid` -- the stable identifier USJ puts on every
            // chapter and verse -- is derived from the book code, so a chapter
            // parsed alone yields " 1:1" where the document yields "GEN 1:1".
            // Every reference in the incremental path would be missing its
            // book, and Go to Reference (P2.9) reads exactly this field.
            // Discovered by P0.5 against the corpus: it affected 188 of 190
            // files and nothing in P0.4's own tests could see it, because
            // those tests compared chunked parses against each other.
            //
            // Prepending real context beats patching the one symptom: any
            // other derivation that reaches back to the header is fixed by
            // the same mechanism instead of waiting to be noticed.
            let prefix = match (is_header, self.book_code()) {
                (false, Some(code)) => format!("\\id {code}\n"),
                _ => String::new(),
            };
            let offset = prefix.len();

            let owned;
            let source = if offset == 0 {
                text
            } else {
                owned = format!("{prefix}{text}");
                &owned
            };

            let backend = Backend::parse(source);

            ChunkParse {
                content: backend
                    .tree()
                    .iter()
                    .filter_map(|node| strip_prefix(node, offset))
                    .collect(),
                diagnostics: backend
                    .diagnostics()
                    .into_iter()
                    // Re-derived from the marker table below, with a version
                    // model the parser does not have.
                    .filter(|diagnostic| !severity::is_derived(diagnostic.code))
                    .filter(|diagnostic| is_header || !is_document_scoped(diagnostic.code))
                    // Anything the synthetic context provoked describes text
                    // the user did not write.
                    .filter(|diagnostic| diagnostic.span.end > offset || offset == 0)
                    .map(|diagnostic| Diagnostic {
                        span: unshift_span(&diagnostic.span, offset),
                        ..diagnostic
                    })
                    .collect(),
            }
        })
    }

    /// One chunk's nodes, in document coordinates.
    pub fn chunk_content(&self, index: usize) -> Vec<Node> {
        let offset = self.chunks[index].start;
        self.parse(index)
            .content
            .iter()
            .map(|node| translate(node, offset))
            .collect()
    }

    /// One chunk's diagnostics, in document coordinates.
    ///
    /// Marker conditions are derived here rather than cached with the chunk,
    /// because they depend on the configuration — changing the target version
    /// or suppressing a code must not require reparsing anything.
    pub fn chunk_diagnostics(&self, index: usize) -> Vec<Diagnostic> {
        let offset = self.chunks[index].start;
        let parsed = self.parse(index);

        parsed
            .diagnostics
            .iter()
            .map(|diagnostic| Diagnostic {
                span: shift_span(&diagnostic.span, offset),
                ..diagnostic.clone()
            })
            .chain(severity::marker_diagnostics(
                &self.chunk_content(index),
                &self.source,
                &self.config,
            ))
            .filter(|diagnostic| !self.config.is_suppressed(diagnostic.code))
            .collect()
    }

    /// The whole document's nodes. Parses every chunk not yet parsed.
    pub fn content(&self) -> Vec<Node> {
        (0..self.chunks.len())
            .flat_map(|index| self.chunk_content(index))
            .collect()
    }

    /// Every diagnostic, in source order, including the cross-chunk ones.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut all: Vec<Diagnostic> = (0..self.chunks.len())
            .flat_map(|index| self.chunk_diagnostics(index))
            .chain(
                self.cross_chunk_diagnostics()
                    .into_iter()
                    .filter(|diagnostic| !self.config.is_suppressed(diagnostic.code)),
            )
            .collect();
        all.sort_by_key(|diagnostic| diagnostic.span.start);
        all
    }

    /// Tier 3 — what no single chunk can see.
    ///
    /// Derived from chunk summaries rather than from the tree, so it is
    /// O(chunks) and can run unconditionally after every edit. Duplicate
    /// chapters are all it covers today; verse sequencing and range overlap
    /// are P0.8, and belong here for the same reason.
    fn cross_chunk_diagnostics(&self) -> Vec<Diagnostic> {
        let mut seen: Vec<(u32, usize)> = Vec::new();
        let mut diagnostics = Vec::new();

        for chunk in &self.chunks {
            let Some(number) = chunk.number else { continue };
            if seen.iter().any(|(previous, _)| *previous == number) {
                diagnostics.push(Diagnostic {
                    code: DiagnosticCode::DuplicateChapter,
                    severity: crate::Severity::Error,
                    span: ByteSpan::new(chunk.start, line_end(&self.source, chunk.start)),
                    message: format!("chapter {number} appears more than once"),
                });
            } else {
                seen.push((number, chunk.start));
            }
        }

        diagnostics
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("source_len", &self.source.len())
            .field("chunks", &self.chunks.len())
            .field("rev", &self.rev)
            .field(
                "parsed",
                &self.chunks.iter().filter(|c| c.is_parsed()).count(),
            )
            .finish()
    }
}

// ------------------------------------------------------------- chunking ---

/// Splits `text` at line-initial `\c`, returning chunks offset by `base`.
fn split(text: &str, base: usize, rev: u64) -> Vec<Chunk> {
    let mut starts: Vec<(usize, Option<u32>)> = Vec::new();

    let mut line = 0usize;
    while line < text.len() {
        if let Some(number) = chapter_number(&text[line..]) {
            starts.push((line, number));
        }
        match text[line..].find('\n') {
            Some(offset) => line += offset + 1,
            None => break,
        }
    }

    let mut chunks = Vec::with_capacity(starts.len() + 1);
    let mut push = |number: Option<u32>, start: usize, end: usize| {
        chunks.push(Chunk {
            number,
            start: base + start,
            end: base + end,
            rev,
            parsed: OnceCell::new(),
        });
    };

    match starts.first() {
        // No chapter marker anywhere: the whole thing is header.
        None => push(None, 0, text.len()),
        Some(&(first, _)) => {
            if first > 0 {
                push(None, 0, first);
            }
            for (index, &(start, number)) in starts.iter().enumerate() {
                let end = starts.get(index + 1).map_or(text.len(), |&(next, _)| next);
                push(number, start, end);
            }
        }
    }

    chunks
}

/// The chapter number, if `line` begins a `\c` marker.
///
/// The marker must be exactly `\c` — `\cl`, `\cp`, and `\ca` all begin with
/// the same two characters and none of them is a chapter boundary. Getting
/// this wrong would split a chapter at its own published number and produce
/// two chunks that each parse as nonsense.
///
/// An unnumbered or unparseable `\c` still opens a chunk: it is a
/// synchronization point regardless, and the missing number is a diagnostic
/// rather than a reason to mis-chunk.
fn chapter_number(line: &str) -> Option<Option<u32>> {
    let rest = line.strip_prefix("\\c")?;
    let after = rest.chars().next();
    if !matches!(
        after,
        None | Some(' ') | Some('\t') | Some('\r') | Some('\n')
    ) {
        return None;
    }

    let digits: String = rest
        .trim_start_matches([' ', '\t'])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();

    Some(digits.parse().ok())
}

fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |index| index + 1)
}

fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index + 1)
}

// ------------------------------------------------------------ diagnostics ---

/// Conditions that describe the document rather than a chapter.
///
/// A chapter parsed on its own has no `\id`, so the parser reports one missing
/// — correctly, for the text it was given, and uselessly, because the `\id` is
/// in the header chunk. Suppressed everywhere but the header, where the
/// question is real.
///
/// The sequencing codes are suppressed for a different reason: no chunk can
/// see its neighbours, so any answer it gives is guesswork. They are Tier 3's
/// to report.
const fn is_document_scoped(code: DiagnosticCode) -> bool {
    matches!(
        code,
        DiagnosticCode::MissingIdMarker
            | DiagnosticCode::DuplicateId
            | DiagnosticCode::TextBeforeId
            | DiagnosticCode::HeaderAfterBody
            | DiagnosticCode::BodyParagraphBeforeChapter
            | DiagnosticCode::MissingChapterMarker
            | DiagnosticCode::InvalidChapterSequence
            | DiagnosticCode::InvalidVerseSequence
            | DiagnosticCode::DuplicateChapter
    )
}

// ------------------------------------------------------------ translation ---

fn shift_span(span: &ByteSpan, by: usize) -> ByteSpan {
    ByteSpan::new(span.start + by, span.end + by)
}

fn unshift_span(span: &ByteSpan, by: usize) -> ByteSpan {
    ByteSpan::new(span.start.saturating_sub(by), span.end.saturating_sub(by))
}

/// Removes the synthetic context a chapter chunk was parsed inside.
///
/// A node lying entirely within the prefix describes text the user never
/// wrote — the synthetic `\id` itself — and is dropped along with its
/// children. Everything else moves back by the prefix's length.
fn strip_prefix(node: &Node, offset: usize) -> Option<Node> {
    if offset == 0 {
        return Some(node.clone());
    }
    if node.span.as_ref().is_some_and(|span| span.end <= offset) {
        return None;
    }

    Some(Node {
        kind: node.kind,
        marker: node.marker.clone(),
        attributes: node.attributes.clone(),
        span: node.span.as_ref().map(|span| unshift_span(span, offset)),
        anchor_cst: node.anchor_cst,
        children: node
            .children
            .iter()
            .filter_map(|child| strip_prefix(child, offset))
            .collect(),
        text: node.text.clone(),
        raw: node.raw.as_ref().map(|span| unshift_span(span, offset)),
    })
}

fn translate(node: &Node, by: usize) -> Node {
    Node {
        kind: node.kind,
        marker: node.marker.clone(),
        attributes: node.attributes.clone(),
        span: node.span.as_ref().map(|span| shift_span(span, by)),
        anchor_cst: node.anchor_cst,
        children: node.children.iter().map(|c| translate(c, by)).collect(),
        text: node.text.clone(),
        raw: node.raw.as_ref().map(|span| shift_span(span, by)),
    }
}

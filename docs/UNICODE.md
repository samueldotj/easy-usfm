# Unicode and Complex Scripts

Coordinate spaces, grapheme clusters, visual reordering, normalization, joiners, input methods, digits, fonts, and text direction.

Related: [ARCHITECTURE](ARCHITECTURE.md) · [FILE-FIDELITY](FILE-FIDELITY.md) · [PRODUCT](PRODUCT.md)

---

Scripture is translated into scripts that break the assumptions Latin text makes — that one code point is one character is one visual unit. All three fail, and they fail in ways that surface in cursor movement, selection, search, and rendering.

Three items here are correctness requirements that would otherwise ship as bugs nobody could reproduce on a US keyboard: §3 visual reordering, §4 normalization, §5 input-method composition.

---

## 1. Three coordinate spaces

Exactly three offset spaces exist. Every API, struct field, and variable name declares which one it uses.

This comes first because conflating them is the most likely serious bug in the project — and one ASCII fixtures **cannot** detect, because all three agree on ASCII.

| Space | Unit | Used by | Naming |
|---|---|---|---|
| **Byte** | UTF-8 byte | `easy-usfm-core` internals and `usfm3` | `b_start`, `byte_range` |
| **Char16** | UTF-16 code unit | everything crossing to JavaScript; CodeMirror; DOM ranges | `u16_start`, `Char16Range` |
| **Grapheme** | extended grapheme cluster | user-facing interaction and display | `g_col`, `grapheme_col` |

> **Byte offsets never leave `easy-usfm-core`. Everything serialized to the frontend is Char16. Grapheme is computed for interaction and display and never round-trips.**

Enforced structurally, not by discipline:

```rust
#[derive(Serialize)]
#[serde(transparent)]
pub struct Char16(pub u32);        // no From<usize>; only Utf16Mapper constructs it

pub struct Utf16Mapper { line_starts: Vec<(u32 /*byte*/, u32 /*utf16*/)> }

impl Utf16Mapper {
    pub fn to_char16(&self, byte: usize) -> Char16 { /* binary search + in-line scan */ }
}
```

The byte-offset types have no `Serialize` impl, so **a byte offset cannot reach the frontend without a compile error.** Converting `usfm3`'s byte offsets at the facade is one of that layer's primary jobs.

Grapheme columns use `unicode-segmentation`, computed lazily for the cursor line only.

## 2. Graphemes are the interaction unit

Tamil `க்ஷ` is three code points rendering as one indivisible unit; Devanagari `क्ष` likewise. The user perceives one character; naive code sees three.

> **The grapheme cluster does not always agree with the reader.** Measured in P0.3, and confirmed identical between Rust's `unicode-segmentation` and V8's `Intl.Segmenter`:
>
> | | Renders as | Extended grapheme clusters |
> |---|---|---|
> | Devanagari `क्ष` | one unit | **one** |
> | Tamil `க்ஷ` | one unit | **two** |
>
> Unicode 15.1's rule GB9c keeps consonant–virama–consonant together, but only for viramas carrying `Indic_Conjunct_Break=Linker`. Devanagari's U+094D qualifies; Tamil's U+0BCD does not.
>
> So on Tamil conjuncts, cluster-wise arrow movement stops *inside* what the reader sees as one character — the exact symptom this section exists to prevent, arriving through the mechanism meant to prevent it. It is not a segmentation bug and not ours to fix upstream. The editor has to decide whether the interaction unit is the cluster or something script-aware on top of it, and that decision belongs with P1.2 rather than here. Both implementations agreeing means at least the two sides of the boundary will be wrong in the same way, which is what makes it safe to defer.

| Operation | Unit |
|---|---|
| Arrow keys, selection extension, delete forward | Extended grapheme cluster |
| Double-click selection | Word, via `Intl.Segmenter`, not `\w+` |
| **Backspace** | **One code point** |
| Status bar column, character counts | Grapheme cluster |

**The Backspace exception is deliberate.** Someone who types a consonant then the wrong vowel sign needs to delete only the vowel sign; cluster-wise Backspace forces retyping the conjunct. A setting (`Backspace deletes: code point / whole cluster`) defaults to code point, since conventions differ by platform.

CodeMirror 6 handles cluster-wise movement natively via `Intl.Segmenter`. **Verify rather than assume** — the failure mode is a cursor that appears stuck or jumps two characters, which users report as "the editor is broken" and nothing more.

## 3. Visual order is not logical order

Devanagari `कि` is stored as क then ि but renders with the vowel sign **to the left** of the consonant. Tamil does the same with ெ, ே, ை. Khmer and Myanmar reorder similarly.

So **"position N is left of position N+1" is false**, in scripts that are otherwise left-to-right.

- **Never compute a cursor position from an x-coordinate arithmetically.** All click-to-position mapping goes through browser hit testing — `posAtCoords`, `caretPositionFromPoint`. Code estimating an index from pixel offset over average character width lands on the wrong side of a reordered mark.
- **Selection highlight can paint as discontinuous rectangles** when a reordered mark falls outside a logically contiguous range. Correct behaviour; must not be "fixed".
- Editor-to-preview mapping is unaffected because it works on offsets, not pixels — provided the first rule holds on both sides.

## 4. Normalization versus byte fidelity

Text circulates in both NFC and NFD. Keyboards, input methods, and export tools disagree, and the same word can have two byte representations that render identically.

This collides with the preservation guarantee:

> The buffer is **never** normalized — a file in NFD saves as NFD ([FILE-FIDELITY](FILE-FIDELITY.md)).
>
> But a search for a word gets zero hits because the keyboard produced NFC and the file is NFD, with the word visibly on screen. That is the most infuriating bug this class of application can have.

**Normalize for comparison, never for storage.**

| Operation | Normalization |
|---|---|
| Buffer storage, save, undo history | **None.** Raw bytes preserved. |
| Find and Replace | Both sides compared as NFC; matches mapped back to raw offsets |
| Autocomplete, go-to-reference, diagnostic comparison | NFC |
| Replace *insertion* text | Exactly as typed — not normalized to match surroundings |

`easy-usfm-core` keeps a normalized search index alongside the raw buffer with an offset map, rebuilt on the same dirty-chunk schedule as the parse, so it costs nothing extra.

**`USFM-I021`** reports mixed forms (Information) with an explicit "Normalize to NFC" command — never automatic. Find and Replace gains a **Match exact byte sequence** toggle, off by default.

## 5. Input-method composition

Mediated input — transliteration, InScript-style layouts, platform IMEs — produces intermediate states before commit.

**This breaks the delta protocol** ([ARCHITECTURE §9](ARCHITECTURE.md#9-delta-protocol)). CodeMirror emits transactions for intermediate states. Sending them means the mirrored buffer receives text the user never committed, the preview flickers through partial syllables, and — worst — some platforms' composition teardown produces no clean inverse transaction, so the mirror desyncs and the checksum forces a full resync mid-typing, on every word.

```typescript
let composing = false;
view.dom.addEventListener("compositionstart", () => { composing = true; });
view.dom.addEventListener("compositionend",   () => {
  composing = false;
  flushPendingEdits();               // one coalesced batch for the committed text
});

// In the update listener:
if (composing) { bufferEdit(update.changes); return; }
```

Parse and preview suspend for the composition and resume on commit; the editor itself stays responsive. **This lands with the delta protocol, not after it.**

## 6. Non-ASCII digits

Many scripts have their own digit sets, used in published verse numbering.

- **`\v` numbers stay ASCII** — that is the USFM data model. Non-ASCII digits there raise `USFM-E018`.
- **`\vp` and `\cp`** carry them and render as-is.
- **Go to Reference accepts both.** `௩:௧` and `3:1` resolve identically, via Unicode `Numeric_Value` rather than a hardcoded table, so every script works without enumeration.

## 7. Fonts

**Monospace coverage is poor to nonexistent** for many complex scripts. A monospace-first editor stack renders them as tofu or falls back unpredictably per platform. So the conventional editor font choice is inverted:

- **Content font: proportional, script-appropriate, user-selectable.** Column alignment in the content area is abandoned as a goal — unachievable for these scripts and not needed.
- **Gutter and line numbers: monospace, always**, independent of the content font. This preserves the only alignment that matters.

The default stack lists Noto families covering the corpus scripts, falling through to `system-ui`. Fonts are **not bundled** — licensing, and roughly 10 MB per script.

- **Per-script size multiplier**, default **1.15** for scripts with dense conjuncts or stacked marks. They need 110–120 % of a Latin face's point size for equivalent legibility, because the meaningful detail is in the marks. One global font size gives text comfortable in English and cramped elsewhere.
- **Line height 1.7** for scripts with marks above and below the baseline, 1.5 for Latin. Default leading causes visible collision.
- **Missing-font detection** on open, with a one-time non-modal notice naming the script and linking to a download.

## 8. Text direction

Right-to-left **interface** mirroring is out of scope for v1.0 ([PRODUCT §3](PRODUCT.md#3-scope)) — it is a week of work plus a permanent four-way test matrix, and it is deferrable in a way that document support is not.

**Document** direction is in scope and fully plumbed:

```typescript
EditorView.contentAttributes.of({ dir: documentDirection })   // explicit, not inherited
EditorView.perLineTextDirection.of(true)
```

with a **Text direction: Auto / LTR / RTL** control in the status bar. The explicit setting exists because auto-detection on a line beginning with `\v 1` guesses LTR and would be wrong for a whole RTL document.

Marker tokens carry `unicode-bidi: isolate; direction: ltr` in their syntax decoration. This is **rendering-only** — it changes bidi resolution for the marker run without inserting FSI/PDI characters, so source bytes are untouched and byte fidelity holds. Inserting real isolate characters would corrupt the file and must never be done.

**One hedge, free:** logical CSS properties from the first commit, lint-enforced.

```json
"csstools/use-logical": [true, { "except": ["float"] }]
```

`margin-inline-start` not `margin-left`; `text-align: start` not `left`. Same keystrokes, complete browser support, and the single factor determining whether adding interface mirroring later costs a week or a month — retrofitting means auditing every stylesheet written in the interim.

---

## 9. Testing

### 9.1 Offset property tests

Under `proptest`, over a mixed-script alphabet: ASCII, conjuncts (`க்ஷ`, `क्ष`), vowel signs (`कि`), Khmer and Myanmar clusters, Hebrew and Arabic, combining marks (`e` + U+0301), astral characters, and joiner sequences.

1. `to_char16` is monotonic and never lands mid-surrogate.
2. Slicing the source in JavaScript with a reported Char16 range yields **exactly** the text Rust reports as that span.
3. `byte → char16 → byte` is identity for all character boundaries.
4. Grapheme boundaries agree between Rust's `unicode-segmentation` and the browser's `Intl.Segmenter`.

Test 2 catches real bugs; 1 and 3 can pass on an implementation that is internally consistent and wrong.

### 9.2 Regression fixtures

Every wrong-cursor-position bug becomes a permanent fixture in `tests/offsets/` recording the document, the edit, and the expected Char16 span. **This class of bug recurs; the fixture set is the only defence.**

### 9.3 Corpus

Script coverage and the pathological set are in [ARCHITECTURE §12.4](ARCHITECTURE.md#124-corpus). The cases that exist specifically for this document: the same file in NFC and NFD (§4 end to end); deliberate joiners including one inside a marker name; `\vp` with non-ASCII digits; long conjunct chains; marks above and below on consecutive lines.

### 9.4 Rendering

Font fallback cannot be validated in CI. **Shaping is verified manually on real Windows and macOS installations before release**, since platform font coverage only exists there.

---

## Appendix — zero-width joiners

U+200D and U+200C control conjunct formation in several scripts. They change rendering, carry meaning, are invisible, and are trivially inserted or deleted by accident.

- Rendered as small distinct glyphs, toggled by **View → Show invisible characters**, defaulting to on when the document's script uses them.
- **`USFM-W022`** — joiner inside a marker name. Almost certainly accidental, and otherwise produces an unknown-marker error whose cause is invisible on screen.
- **`USFM-I023`** — joiner adjacent to a marker boundary.
- Find and Replace treats them as significant by default, with an **Ignore joiners** toggle.

A file that looks correct and parses wrong is otherwise unexplainable to the user.

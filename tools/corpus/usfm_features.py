"""Script and feature detection for USFM files.

Shared by classify.py, select.py, and verify.py. Standard library only —
these run in CI on every push and must not need a package install.

Two questions are answered here:

  detect_scripts(text)   which writing systems does this file use?
  detect_features(text)  which USFM construct classes does it exercise?

Together they drive corpus selection: we want the smallest set of files that
covers every required script and every feature class, because that set is what
gets committed to the repository and run on every push.
"""

from __future__ import annotations

import re
import unicodedata
from collections import Counter

# --------------------------------------------------------------------------
# Scripts
# --------------------------------------------------------------------------

# Unicode character names begin with the script name, so the standard library
# gives us script detection without a dependency: "TAMIL LETTER A" -> Tamil.
# Prefixes are checked longest-first, so "CJK UNIFIED" wins over "CJK".
_SCRIPT_PREFIXES: list[tuple[str, str]] = [
    ("CJK UNIFIED", "Han"),
    ("CJK COMPATIBILITY", "Han"),
    ("HANGUL", "Hangul"),
    ("HIRAGANA", "Kana"),
    ("KATAKANA", "Kana"),
    ("LATIN", "Latin"),
    ("GREEK", "Greek"),
    ("COPTIC", "Coptic"),
    ("CYRILLIC", "Cyrillic"),
    ("HEBREW", "Hebrew"),
    ("ARABIC", "Arabic"),
    ("SYRIAC", "Syriac"),
    ("THAANA", "Thaana"),
    ("DEVANAGARI", "Devanagari"),
    ("BENGALI", "Bengali"),
    ("GURMUKHI", "Gurmukhi"),
    ("GUJARATI", "Gujarati"),
    ("ORIYA", "Oriya"),
    ("TAMIL", "Tamil"),
    ("TELUGU", "Telugu"),
    ("KANNADA", "Kannada"),
    ("MALAYALAM", "Malayalam"),
    ("SINHALA", "Sinhala"),
    ("THAI", "Thai"),
    ("LAO", "Lao"),
    ("TIBETAN", "Tibetan"),
    ("MYANMAR", "Myanmar"),
    ("GEORGIAN", "Georgian"),
    ("ARMENIAN", "Armenian"),
    ("ETHIOPIC", "Ethiopic"),
    ("CHEROKEE", "Cherokee"),
    ("KHMER", "Khmer"),
    ("MONGOLIAN", "Mongolian"),
    ("JAVANESE", "Javanese"),
    ("BALINESE", "Balinese"),
    ("TIFINAGH", "Tifinagh"),
    ("NKO", "Nko"),
    ("VAI", "Vai"),
    ("YI ", "Yi"),
]

# Scripts the committed corpus must cover, chosen to exercise combining marks,
# conjunct formation, visual reordering, right-to-left, and the absence of word
# spacing. See docs/ARCHITECTURE.md section 12.4.
REQUIRED_SCRIPTS: frozenset[str] = frozenset({
    "Latin", "Greek", "Cyrillic", "Hebrew", "Arabic", "Devanagari",
    "Tamil", "Bengali", "Thai", "Khmer", "Myanmar", "Han",
})

# Ignore the punctuation and digits that appear in every file regardless of
# language, so a Latin-punctuated Thai text is not reported as bilingual.
_IGNORED_CATEGORIES = frozenset({"Zs", "Cc", "Cf", "Po", "Pd", "Ps", "Pe",
                                 "Pi", "Pf", "Pc", "Sm", "Sk", "Nd", "No"})


def detect_scripts(text: str, min_share: float = 0.01) -> set[str]:
    """Scripts present in *text*, ignoring runs below *min_share* of characters.

    The threshold keeps a stray Latin book code or a single Greek word from
    counting as script coverage — we want scripts the file actually exercises.
    """
    counts: Counter[str] = Counter()
    for ch in text:
        if unicodedata.category(ch) in _IGNORED_CATEGORIES:
            continue
        try:
            name = unicodedata.name(ch)
        except ValueError:            # unnamed control or private use
            continue
        for prefix, script in _SCRIPT_PREFIXES:
            if name.startswith(prefix):
                counts[script] += 1
                break

    total = sum(counts.values())
    if total == 0:
        return set()
    return {s for s, n in counts.items() if n / total >= min_share}


# --------------------------------------------------------------------------
# Features
# --------------------------------------------------------------------------

# A feature class is a construct family the parser and preview must handle.
# Selection requires every class to appear somewhere in the committed corpus,
# so that a change to note handling cannot pass CI without a file that has notes.
_FEATURE_PATTERNS: dict[str, re.Pattern[str]] = {
    "notes":          re.compile(r"\\(?:f|fe|ef|efe|x|ex)\s"),
    "poetry":         re.compile(r"\\q[acdmr\d]?\d?\s"),
    "lists":          re.compile(r"\\(?:li\d?|lh|lf|lim\d?)\s"),
    "tables":         re.compile(r"\\(?:tr|th\d?|thr\d?|thc\d?|tc\d?|tcr\d?|tcc\d?)\s"),
    "milestones":     re.compile(r"\\[a-z]+-[se]\s*\\?\*"),
    "attributes":     re.compile(r"\|\s*[a-z-]+\s*="),
    "sidebars":       re.compile(r"\\esb\b"),
    "figures":        re.compile(r"\\fig\b"),
    "introductions":  re.compile(r"\\(?:imt\d?|is\d?|ip|ipi|im|iot|io\d?|iex)\s"),
    "peripherals":    re.compile(r"\\periph\b"),
    "custom_z":       re.compile(r"\\z[a-zA-Z]"),
    "titles":         re.compile(r"\\(?:mt\d?|ms\d?|s\d?|sr|d|sp|cl|cd)\s"),
    "char_styles":    re.compile(r"\\\+?(?:bd|it|bdit|em|sc|no|nd|wj|add|k|w|tl|pn|qt)\s"),
    "alt_numbering":  re.compile(r"\\(?:va|vp|ca|cp)\s"),
    "verse_ranges":   re.compile(r"\\v\s+\d+[-\u2013]\d+"),
    "nested_markers": re.compile(r"\\\+[a-z]"),
}

FEATURE_CLASSES: frozenset[str] = frozenset(_FEATURE_PATTERNS)


def detect_features(text: str) -> set[str]:
    """USFM feature classes exercised by *text*."""
    return {name for name, pat in _FEATURE_PATTERNS.items() if pat.search(text)}


# --------------------------------------------------------------------------
# Encoding traits — recorded so the corpus provably covers them
# --------------------------------------------------------------------------

def detect_traits(raw: bytes) -> set[str]:
    """Byte-level traits relevant to fidelity testing."""
    traits: set[str] = set()
    if raw.startswith(b"\xef\xbb\xbf"):
        traits.add("bom")
    body = raw[3:] if "bom" in traits else raw

    crlf = body.count(b"\r\n")
    lf = body.count(b"\n") - crlf
    cr = body.count(b"\r") - crlf
    if crlf and (lf or cr):
        traits.add("mixed_eol")
    elif crlf:
        traits.add("crlf")
    elif cr:
        traits.add("cr")
    else:
        traits.add("lf")

    if body and not body.endswith((b"\n", b"\r")):
        traits.add("no_final_newline")

    try:
        text = body.decode("utf-8")
    except UnicodeDecodeError:
        traits.add("invalid_utf8")
        return traits

    if text != unicodedata.normalize("NFC", text):
        traits.add("not_nfc")
    if "\u200c" in text or "\u200d" in text:
        traits.add("joiners")

    return traits


TRAIT_CLASSES: frozenset[str] = frozenset({
    "bom", "crlf", "lf", "mixed_eol", "no_final_newline", "not_nfc", "joiners",
})


def read_text(path) -> str:
    """Decode a USFM file for analysis, tolerating a BOM and bad bytes."""
    raw = open(path, "rb").read()
    if raw.startswith(b"\xef\xbb\xbf"):
        raw = raw[3:]
    return raw.decode("utf-8", errors="replace")

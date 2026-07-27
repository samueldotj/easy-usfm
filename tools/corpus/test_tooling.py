#!/usr/bin/env python3
"""Self-test for the corpus tooling. Standard library only; no network.

    python3 tools/corpus/test_tooling.py

Builds a synthetic pool covering every required script, feature class, and
encoding trait, then exercises select.py and verify.py against it — including
each way verification is supposed to fail.

The synthetic files exist to test the *tooling*. They are never committed as
corpus content: real parser bugs are found by published Scripture, not by
fixtures we wrote to pass our own checks.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]

SAMPLES = {
    "latin":      ("Latin",      "In the beginning God created the heavens and the earth."),
    "greek":      ("Greek",      "Ἐν ἀρχῇ ἦν ὁ λόγος καὶ ὁ λόγος ἦν πρὸς τὸν θεόν."),
    "cyrillic":   ("Cyrillic",   "В начале сотворил Бог небо и землю."),
    "hebrew":     ("Hebrew",     "בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ׃"),
    "arabic":     ("Arabic",     "فِي الْبَدْءِ خَلَقَ اللهُ السَّمَاوَاتِ وَالأَرْضَ."),
    "devanagari": ("Devanagari", "आदि में परमेश्वर ने आकाश और पृथ्वी की सृष्टि की। क्षत्रिय कि"),
    "tamil":      ("Tamil",      "ஆதியிலே தேவன் வானத்தையும் பூமியையும் சிருஷ்டித்தார். க்ஷ"),
    "bengali":    ("Bengali",    "আদিতে ঈশ্বর আকাশমণ্ডল ও পৃথিবীর সৃষ্টি করিলেন।"),
    "thai":       ("Thai",       "ในปฐมกาลพระเจ้าทรงเนรมิตสร้างฟ้าและแผ่นดินโลก"),
    "khmer":      ("Khmer",      "កាលដើមដំបូង ព្រះបានបង្កើតផ្ទៃមេឃ និងផែនដី"),
    "myanmar":    ("Myanmar",    "အစအဦး၌ ဘုရားသခင်သည် ကောင်းကင်နှင့် မြေကြီးကို ဖန်ဆင်းတော်မူ၏"),
    "han":        ("Han",        "起初神創造天地。地是空虛混沌。"),
}

FEATURES = {
    "notes":          "\\v 2 Text\\f + \\fr 1.2 \\ft A footnote.\\f*\n",
    "poetry":         "\\q1 A poetic line\n\\q2 indented further\n",
    "lists":          "\\lh Header\n\\li1 An entry\n\\lf Footer\n",
    "tables":         "\\tr \\th1 Head \\th2 Head\n\\tr \\tc1 Cell \\tc2 Cell\n",
    "milestones":     "\\qt-s |who=\"Pilate\"\\*Quoted\\qt-e\\*\n",
    "attributes":     "\\v 4 \\w gracious|lemma=\"grace\" strong=\"G5485\"\\w*\n",
    "sidebars":       "\\esb\n\\ms Sidebar\n\\p Body\n\\esbe\n",
    "figures":        "\\fig Caption|src=\"pic.png\" size=\"span\" ref=\"1.1\"\\fig*\n",
    "introductions":  "\\imt1 Intro title\n\\ip Intro paragraph\n\\iot Outline\n",
    "peripherals":    "\\periph Title Page\n\\p Front matter\n",
    "custom_z":       "\\zaln-s |x-strong=\"H0430\"\\*aligned\\zaln-e\\*\n",
    "titles":         "\\mt1 A Title\n\\s1 A section\n\\d A descriptive title\n",
    "char_styles":    "\\v 5 \\nd Lord\\nd* said \\wj words\\wj*\n",
    "alt_numbering":  "\\va 3\\va* \\vp ३\\vp*\n",
    "verse_ranges":   "\\v 6-7 A bridged verse.\n",
    "nested_markers": "\\v 8 \\f + \\ft note with \\+it italic\\+it*\\f*\n",
}


def build_pool(root: Path) -> None:
    """A candidate pool laid out the way fetch.py produces one."""
    names = list(FEATURES)
    prov: dict[str, dict] = {}

    for i, (key, (script, line)) in enumerate(SAMPLES.items()):
        tid = f"{key}test"
        (root / tid).mkdir(parents=True, exist_ok=True)
        prov[tid] = {
            "source": f"https://ebible.org/Scriptures/{tid}_usfm.zip",
            "language": key.title(),
            "script": script,
            "direction": "rtl" if key in ("hebrew", "arabic") else "ltr",
            "copyright": "Public Domain (synthetic fixture)",
            "redistributable": "True",
        }
        for j in range(6):
            text = f"\\id GEN\n\\h Test\n\\mt1 {script}\n\\c 1\n\\p\n\\v 1 {line}\n"
            for k in range((i + j) % 5 + 2):
                text += FEATURES[names[(i * 3 + j * 2 + k) % len(names)]]

            n = i * 6 + j
            if n % 8 == 0:                                    # not_nfc
                text += "\\v 99 cafe\u0301 nai\u0308ve\n"
            if n % 6 == 0:                                    # joiners
                text = text.replace("Text", "Te\u200cxt") + "\\v 98 \u200dzwj\n"
            if n % 5 == 0:                                    # crlf
                text = text.replace("\n", "\r\n")
            if n % 11 == 0:                                   # mixed_eol
                text = text.replace("\n", "\r\n", 3)
            data = text.encode() if n % 9 else text.rstrip("\r\n").encode()
            if n % 7 == 0:                                    # bom
                data = b"\xef\xbb\xbf" + data
            (root / tid / f"{key}{j}.usfm").write_bytes(data)

    (root / "provenance.json").write_text(json.dumps(prov, indent=2), encoding="utf-8")


def run(script: str, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(HERE / script), *args],
                          capture_output=True, text=True)


class Failed(Exception):
    pass


def check(label: str, cond: bool, detail: str = "") -> None:
    print(f"  {'ok  ' if cond else 'FAIL'}  {label}")
    if not cond:
        raise Failed(f"{label}\n{detail}")


def main() -> int:
    tmp = Path(tempfile.mkdtemp(prefix="usfm-corpus-test-"))
    pool, corpus = tmp / "pool", tmp / "corpus"
    try:
        build_pool(pool)
        print(f"pool: {len(list(pool.rglob('*.usfm')))} files in {len(SAMPLES)} translations\n")

        print("coverage")
        r = run("classify.py", str(pool), "--coverage")
        check("pool covers every script, feature, and trait", r.returncode == 0,
              r.stdout + r.stderr)

        print("\nselection")
        r = run("select.py", str(pool), "--target", "24",
                "--manifest", str(corpus / "manifest.toml"),
                "--copy-to", str(corpus / "core"))
        check("select.py succeeds", r.returncode == 0, r.stdout + r.stderr)
        manifest = corpus / "manifest.toml"
        check("manifest written", manifest.exists())
        n = manifest.read_text(encoding="utf-8").count("[[file]]")
        check(f"manifest has 24 entries (got {n})", n == 24)
        check("greedy cover is smaller than the target",
              "greedy cover: " in r.stderr and
              int(r.stderr.split("greedy cover: ")[1].split()[0]) < 24, r.stderr)

        print("\nverification — clean tree")
        r = run("verify.py", "--corpus", str(corpus))
        check("verify passes", r.returncode == 0, r.stdout + r.stderr)

        print("\nverification — each failure mode")
        target = sorted((corpus / "core").glob("*.usfm"))[0]
        original = target.read_bytes()

        target.write_bytes(original + b"x")
        r = run("verify.py", "--corpus", str(corpus))
        check("tampered file rejected",
              r.returncode == 1 and "sha256 mismatch" in r.stdout, r.stdout)
        target.write_bytes(original)

        target.unlink()
        r = run("verify.py", "--corpus", str(corpus))
        check("missing file rejected",
              r.returncode == 1 and "not on disk" in r.stdout, r.stdout)
        target.write_bytes(original)

        orphan = corpus / "core" / "orphan.usfm"
        orphan.write_bytes(original)
        r = run("verify.py", "--corpus", str(corpus))
        check("unlisted file rejected",
              r.returncode == 1 and "not in the manifest" in r.stdout, r.stdout)
        orphan.unlink()

        text = manifest.read_text(encoding="utf-8")
        manifest.write_text(text.replace('redistributable = "True"',
                                         'redistributable = "False"', 1), encoding="utf-8")
        r = run("verify.py", "--corpus", str(corpus))
        check("non-redistributable entry rejected",
              r.returncode == 1 and "must not be committed" in r.stdout, r.stdout)
        manifest.write_text(text, encoding="utf-8")

        blocks = text.split("[[file]]")
        kept = [b for b in blocks[1:] if "tamil" not in b]
        manifest.write_text(blocks[0] + "[[file]]".join([""] + kept), encoding="utf-8")
        for f in (corpus / "core").glob("tamil*"):
            f.unlink()
        r = run("verify.py", "--corpus", str(corpus))
        check("coverage hole rejected",
              r.returncode == 1 and "does not cover script(s): Tamil" in r.stdout, r.stdout)
        r = run("verify.py", "--corpus", str(corpus), "--skip-coverage")
        check("--skip-coverage tolerates the same hole", r.returncode == 0, r.stdout)

        print("\nall checks passed")
        return 0

    except Failed as e:
        print(f"\nFAILED: {e}", file=sys.stderr)
        return 1
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    # Guard against a stdlib change silently breaking normalization detection.
    assert unicodedata.normalize("NFC", "e\u0301") == "\u00e9"
    raise SystemExit(main())

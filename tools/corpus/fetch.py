#!/usr/bin/env python3
"""Fetch USFM translations from eBible.org into the extended corpus tier.

    tools/corpus/fetch.py --list                 # what is available, redistributable
    tools/corpus/fetch.py --dry-run              # what would be downloaded
    tools/corpus/fetch.py                        # download the extended tier
    tools/corpus/fetch.py --ids engwebp,hin2017  # download specific translations

Only translations eBible marks Redistributable *and* downloadable are ever
fetched. That flag is the licence gate: it is published by the distributor per
translation, so the audit is evidence-gathering rather than interpretation.
The copyright line is recorded alongside every file.

Nothing fetched here is committed. The extended tier is gitignored and runs
nightly; the committed core tier is selected from it by select.py.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import sys
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

CATALOG_URL = "https://ebible.org/Scriptures/translations.csv"
ZIP_URL = "https://ebible.org/Scriptures/{id}_usfm.zip"
USER_AGENT = "easy-usfm-corpus/1.0 (+https://github.com/samueldotj/easy-usfm)"

REPO = Path(__file__).resolve().parents[2]
EXTENDED = REPO / "corpus" / "extended"
CATALOG_CACHE = REPO / "corpus" / ".catalog.csv"


def fetch(url: str, timeout: int = 60) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read()


def load_catalog(refresh: bool = False) -> list[dict]:
    """The eBible translation catalog, cached locally."""
    if refresh or not CATALOG_CACHE.exists():
        CATALOG_CACHE.parent.mkdir(parents=True, exist_ok=True)
        CATALOG_CACHE.write_bytes(fetch(CATALOG_URL))
    text = CATALOG_CACHE.read_text(encoding="utf-8-sig")
    return list(csv.DictReader(io.StringIO(text)))


def is_usable(row: dict) -> bool:
    """Redistributable and downloadable, with actual content."""
    if row.get("Redistributable", "").strip().lower() != "true":
        return False
    if row.get("downloadable", "").strip().lower() != "true":
        return False
    books = sum(int(row.get(k) or 0) for k in ("OTbooks", "NTbooks", "DCbooks"))
    return books > 0


def download_translation(tid: str, dest: Path) -> list[Path]:
    """Download and extract one translation's USFM files. Returns written paths."""
    url = ZIP_URL.format(id=tid)
    try:
        blob = fetch(url)
    except urllib.error.HTTPError as e:
        print(f"  {tid}: HTTP {e.code} — skipped", file=sys.stderr)
        return []
    except urllib.error.URLError as e:
        print(f"  {tid}: {e.reason} — skipped", file=sys.stderr)
        return []

    out = dest / tid
    out.mkdir(parents=True, exist_ok=True)
    written: list[Path] = []
    with zipfile.ZipFile(io.BytesIO(blob)) as z:
        for name in z.namelist():
            if not name.lower().endswith((".usfm", ".sfm")):
                continue
            target = out / Path(name).name          # flatten; ignore archive paths
            target.write_bytes(z.read(name))
            written.append(target)
    return written


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--list", action="store_true", help="list usable translations and exit")
    ap.add_argument("--dry-run", action="store_true", help="show what would be fetched")
    ap.add_argument("--ids", help="comma-separated translation ids")
    ap.add_argument("--limit", type=int, default=60,
                    help="max translations to fetch (default 60)")
    ap.add_argument("--refresh-catalog", action="store_true")
    ap.add_argument("--dest", type=Path, default=EXTENDED)
    args = ap.parse_args()

    try:
        rows = load_catalog(refresh=args.refresh_catalog)
    except urllib.error.URLError as e:
        print(f"cannot reach {CATALOG_URL}: {e.reason}", file=sys.stderr)
        return 2

    usable = [r for r in rows if is_usable(r)]
    print(f"catalog: {len(rows)} translations, {len(usable)} redistributable and downloadable",
          file=sys.stderr)

    if args.ids:
        wanted = {i.strip() for i in args.ids.split(",")}
        selected = [r for r in usable if r["translationId"] in wanted]
        unknown = wanted - {r["translationId"] for r in selected}
        for u in sorted(unknown):
            print(f"  {u}: not redistributable, not downloadable, or unknown",
                  file=sys.stderr)
    else:
        # One translation per script, largest first, so the extended tier spreads
        # across writing systems instead of piling up on Latin.
        by_script: dict[str, list[dict]] = {}
        for r in usable:
            by_script.setdefault(r.get("script") or "Unknown", []).append(r)
        selected = []
        for script, group in sorted(by_script.items()):
            group.sort(key=lambda r: -sum(int(r.get(k) or 0)
                                          for k in ("OTbooks", "NTbooks", "DCbooks")))
            selected += group[:max(1, args.limit // max(1, len(by_script)))]
        selected = selected[:args.limit]

    if args.list:
        for r in sorted(usable, key=lambda r: (r.get("script") or "", r["translationId"])):
            print(f"{r['translationId']:<12} {r.get('script',''):<12} "
                  f"{r.get('textDirection',''):<4} {r.get('languageNameInEnglish','')}")
        return 0

    print(f"selected {len(selected)} translations", file=sys.stderr)
    if args.dry_run:
        for r in selected:
            print(f"{r['translationId']:<12} {r.get('script',''):<12} "
                  f"{ZIP_URL.format(id=r['translationId'])}")
        return 0

    args.dest.mkdir(parents=True, exist_ok=True)
    provenance: dict[str, dict] = {}
    total = 0
    for i, r in enumerate(selected, 1):
        tid = r["translationId"]
        print(f"[{i}/{len(selected)}] {tid}", file=sys.stderr)
        written = download_translation(tid, args.dest)
        if not written:
            continue
        total += len(written)
        provenance[tid] = {
            "source": ZIP_URL.format(id=tid),
            "language": r.get("languageNameInEnglish", ""),
            "script": r.get("script", ""),
            "direction": r.get("textDirection", ""),
            "copyright": r.get("Copyright", ""),
            "redistributable": r.get("Redistributable", ""),
            "files": len(written),
        }

    (args.dest / "provenance.json").write_text(
        json.dumps(provenance, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"\n{total} files from {len(provenance)} translations in {args.dest}",
          file=sys.stderr)
    print("provenance written to provenance.json", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Report the scripts, features, and encoding traits of USFM files.

    tools/corpus/classify.py corpus/core            # table
    tools/corpus/classify.py corpus/core --json     # machine-readable
    tools/corpus/classify.py corpus/core --coverage # what is missing

Used to decide what a candidate file would add to the corpus, and to explain
why a given file is in it.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from usfm_features import (  # noqa: E402
    FEATURE_CLASSES, REQUIRED_SCRIPTS, TRAIT_CLASSES,
    detect_features, detect_scripts, detect_traits, read_text,
)

USFM_SUFFIXES = {".usfm", ".sfm", ".SFM", ".USFM"}


def usfm_files(paths: list[Path]) -> list[Path]:
    found: list[Path] = []
    for p in paths:
        if p.is_dir():
            found += [f for f in sorted(p.rglob("*")) if f.suffix in USFM_SUFFIXES]
        elif p.suffix in USFM_SUFFIXES:
            found.append(p)
    return found


def classify(path: Path) -> dict:
    raw = path.read_bytes()
    text = read_text(path)
    return {
        "path": str(path),
        "bytes": len(raw),
        "scripts": sorted(detect_scripts(text)),
        "features": sorted(detect_features(text)),
        "traits": sorted(detect_traits(raw)),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--json", action="store_true", help="emit JSON")
    ap.add_argument("--coverage", action="store_true",
                    help="summarise coverage and list what is missing")
    args = ap.parse_args()

    files = usfm_files(args.paths)
    if not files:
        print("no USFM files found", file=sys.stderr)
        return 1

    results = [classify(f) for f in files]

    if args.json:
        json.dump(results, sys.stdout, indent=2, ensure_ascii=False)
        print()
        return 0

    if args.coverage:
        return report_coverage(results)

    width = max(len(Path(r["path"]).name) for r in results)
    for r in results:
        print(f"{Path(r['path']).name:<{width}}  "
              f"{r['bytes']//1024:>5} KB  "
              f"{','.join(r['scripts']) or '-':<28}  "
              f"{len(r['features']):>2} features  "
              f"{','.join(r['traits'])}")
    return 0


def report_coverage(results: list[dict]) -> int:
    scripts = {s for r in results for s in r["scripts"]}
    features = {f for r in results for f in r["features"]}
    traits = {t for r in results for t in r["traits"]}

    missing_scripts = REQUIRED_SCRIPTS - scripts
    missing_features = FEATURE_CLASSES - features
    missing_traits = TRAIT_CLASSES - traits

    print(f"files      {len(results)}")
    print(f"scripts    {len(scripts & REQUIRED_SCRIPTS)}/{len(REQUIRED_SCRIPTS)} required"
          f"  ({len(scripts)} seen in total)")
    print(f"features   {len(features)}/{len(FEATURE_CLASSES)}")
    print(f"traits     {len(traits)}/{len(TRAIT_CLASSES)}")

    ok = True
    for label, missing in (("scripts", missing_scripts),
                           ("features", missing_features),
                           ("traits", missing_traits)):
        if missing:
            ok = False
            print(f"\nmissing {label}: {', '.join(sorted(missing))}")

    if ok:
        print("\ncoverage complete")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())

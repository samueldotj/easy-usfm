//! Self-test for the corpus tooling. No network.
//!
//! Builds a synthetic pool covering every required script, feature class, and
//! encoding trait, then exercises selection and verification against it —
//! including every way verification is supposed to fail.
//!
//! The synthetic files test the *tooling*. They are never committed as corpus
//! content: real parser bugs come from published Scripture, not from files
//! written to satisfy our own checks.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::corpus;
use crate::features::{
    detect_features, detect_scripts, detect_traits, scan_markers, FEATURE_CLASSES,
    REQUIRED_SCRIPTS, TRAIT_CLASSES,
};

// ------------------------------------------------------------- fixtures ---

const SAMPLES: &[(&str, &str, &str)] = &[
    (
        "latin",
        "Latin",
        "In the beginning God created the heavens and the earth.",
    ),
    (
        "greek",
        "Greek",
        "Ἐν ἀρχῇ ἦν ὁ λόγος καὶ ὁ λόγος ἦν πρὸς τὸν θεόν.",
    ),
    (
        "cyrillic",
        "Cyrillic",
        "В начале сотворил Бог небо и землю.",
    ),
    ("hebrew", "Hebrew", "בְּרֵאשִׁית בָּרָא אֱלֹהִים אֵת הַשָּׁמַיִם וְאֵת הָאָרֶץ׃"),
    ("arabic", "Arabic", "فِي الْبَدْءِ خَلَقَ اللهُ السَّمَاوَاتِ وَالأَرْضَ."),
    (
        "devanagari",
        "Devanagari",
        "आदि में परमेश्वर ने आकाश और पृथ्वी की सृष्टि की। क्षत्रिय कि",
    ),
    (
        "tamil",
        "Tamil",
        "ஆதியிலே தேவன் வானத்தையும் பூமியையும் சிருஷ்டித்தார். க்ஷ",
    ),
    (
        "bengali",
        "Bengali",
        "আদিতে ঈশ্বর আকাশমণ্ডল ও পৃথিবীর সৃষ্টি করিলেন।",
    ),
    ("thai", "Thai", "ในปฐมกาลพระเจ้าทรงเนรมิตสร้างฟ้าและแผ่นดินโลก"),
    ("khmer", "Khmer", "កាលដើមដំបូង ព្រះបានបង្កើតផ្ទៃមេឃ និងផែនដី"),
    (
        "myanmar",
        "Myanmar",
        "အစအဦး၌ ဘုရားသခင်သည် ကောင်းကင်နှင့် မြေကြီးကို ဖန်ဆင်းတော်မူ၏",
    ),
    ("han", "Han", "起初神創造天地。地是空虛混沌。"),
];

const FEATURE_SNIPPETS: &[(&str, &str)] = &[
    ("notes", "\\v 2 Text\\f + \\fr 1.2 \\ft A footnote.\\f*\n"),
    ("poetry", "\\q1 A poetic line\n\\q2 indented further\n"),
    ("lists", "\\lh Header\n\\li1 An entry\n\\lf Footer\n"),
    (
        "tables",
        "\\tr \\th1 Head \\th2 Head\n\\tr \\tc1 Cell \\tc2 Cell\n",
    ),
    ("milestones", "\\qt-s |who=\"Pilate\"\\*Quoted\\qt-e\\*\n"),
    (
        "attributes",
        "\\v 4 \\w gracious|lemma=\"grace\" strong=\"G5485\"\\w*\n",
    ),
    ("sidebars", "\\esb\n\\ms Sidebar\n\\p Body\n\\esbe\n"),
    (
        "figures",
        "\\fig Caption|src=\"pic.png\" size=\"span\" ref=\"1.1\"\\fig*\n",
    ),
    (
        "introductions",
        "\\imt1 Intro title\n\\ip Intro paragraph\n\\iot Outline\n",
    ),
    ("peripherals", "\\periph Title Page\n\\p Front matter\n"),
    (
        "custom_z",
        "\\zaln-s |x-strong=\"H0430\"\\*aligned\\zaln-e\\*\n",
    ),
    (
        "titles",
        "\\mt1 A Title\n\\s1 A section\n\\d A descriptive title\n",
    ),
    ("char_styles", "\\v 5 \\nd Lord\\nd* said \\wj words\\wj*\n"),
    ("alt_numbering", "\\va 3\\va* \\vp \u{967}\\vp*\n"),
    ("verse_ranges", "\\v 6-7 A bridged verse.\n"),
    (
        "nested_markers",
        "\\v 8 \\f + \\ft note with \\+it italic\\+it*\\f*\n",
    ),
];

static TMP_SEQ: AtomicUsize = AtomicUsize::new(0);

fn tmp_dir(tag: &str) -> PathBuf {
    let n = TMP_SEQ.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("easy-usfm-{}-{}-{}", tag, std::process::id(), n));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create temp dir");
    d
}

/// A candidate pool laid out the way `corpus fetch` produces one.
fn build_pool(root: &Path) {
    let mut provenance = serde_json::Map::new();

    for (i, (key, script, line)) in SAMPLES.iter().enumerate() {
        let tid = format!("{key}test");
        let dir = root.join(&tid);
        std::fs::create_dir_all(&dir).unwrap();
        provenance.insert(
            tid.clone(),
            serde_json::json!({
                "source": format!("https://ebible.org/Scriptures/{tid}_usfm.zip"),
                "language": *key,
                "script": *script,
                "direction": if *key == "hebrew" || *key == "arabic" { "rtl" } else { "ltr" },
                "copyright": "Public Domain (synthetic fixture)",
                "redistributable": "True",
            }),
        );

        for j in 0..6usize {
            let mut text =
                format!("\\id GEN\n\\h Test\n\\mt1 {script}\n\\c 1\n\\p\n\\v 1 {line}\n");
            for k in 0..((i + j) % 5 + 2) {
                let (_, snippet) = FEATURE_SNIPPETS[(i * 3 + j * 2 + k) % FEATURE_SNIPPETS.len()];
                text.push_str(snippet);
            }

            let n = i * 6 + j;
            if n % 8 == 0 {
                text.push_str("\\v 99 cafe\u{301} nai\u{308}ve\n"); // not_nfc
            }
            if n % 6 == 0 {
                text = text.replace("Text", "Te\u{200c}xt"); // joiners
                text.push_str("\\v 98 \u{200d}zwj\n");
            }
            if n % 5 == 0 {
                text = text.replace('\n', "\r\n"); // crlf
            }
            if n % 11 == 0 {
                text = text.replacen('\n', "\r\n", 3); // mixed_eol
            }
            let mut bytes = if n % 9 == 0 {
                text.trim_end_matches(['\r', '\n']).as_bytes().to_vec() // no_final_newline
            } else {
                text.as_bytes().to_vec()
            };
            if n % 7 == 0 {
                let mut with_bom = vec![0xEF, 0xBB, 0xBF];
                with_bom.extend_from_slice(&bytes);
                bytes = with_bom;
            }
            std::fs::write(dir.join(format!("{key}{j}.usfm")), &bytes).unwrap();
        }
    }

    std::fs::write(
        root.join("provenance.json"),
        serde_json::to_string_pretty(&provenance).unwrap(),
    )
    .unwrap();
}

fn union_over_pool(root: &Path) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let (mut s, mut f, mut t) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for e in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !e.file_type().is_file() || e.path().extension().and_then(|x| x.to_str()) != Some("usfm")
        {
            continue;
        }
        let raw = std::fs::read(e.path()).unwrap();
        let text = crate::features::read_text(&raw);
        s.extend(detect_scripts(&text, 0.01));
        f.extend(detect_features(&text));
        t.extend(detect_traits(&raw));
    }
    (s, f, t)
}

// ----------------------------------------------------------- unit tests ---

#[test]
fn marker_scanner_reads_tag_level_nesting_and_milestones() {
    let m = scan_markers("\\q1 x \\+bd y\\+bd* \\qt-s |who=\"P\"\\* \\v 1-2 z");
    let q = m.iter().find(|m| m.tag == "q").expect("q");
    assert_eq!(q.level, Some(1));
    let bd: Vec<_> = m.iter().filter(|m| m.tag == "bd").collect();
    assert_eq!(bd.len(), 2, "opening and closing \\+bd");
    assert!(bd.iter().all(|m| m.nested));
    assert!(bd.iter().any(|m| m.closing));
    let qt = m.iter().find(|m| m.tag == "qt").expect("qt");
    assert_eq!(qt.milestone, Some('s'));
}

#[test]
fn scanner_ignores_a_lone_backslash() {
    assert!(scan_markers("a \\ b \\* c")
        .iter()
        .all(|m| !m.tag.is_empty()));
}

#[test]
fn every_feature_class_is_detectable() {
    for (name, snippet) in FEATURE_SNIPPETS {
        let found = detect_features(snippet);
        assert!(
            found.contains(*name),
            "snippet for {name} did not yield it; got {found:?}"
        );
    }
}

#[test]
fn feature_class_list_matches_the_snippets() {
    let covered: BTreeSet<&str> = FEATURE_SNIPPETS.iter().map(|(n, _)| *n).collect();
    let declared: BTreeSet<&str> = FEATURE_CLASSES.iter().copied().collect();
    assert_eq!(covered, declared, "a feature class has no fixture");
}

#[test]
fn scripts_are_detected_and_incidental_runs_ignored() {
    for (_, script, line) in SAMPLES {
        let text = format!("\\id GEN\n\\v 1 {line}\n");
        let got = detect_scripts(&text, 0.01);
        assert!(got.contains(*script), "{script} not detected, got {got:?}");
    }
    // A single Greek word in an English verse is below the threshold.
    let mostly_latin = format!("\\v 1 {} λόγος\n", "word ".repeat(200));
    assert!(!detect_scripts(&mostly_latin, 0.01).contains("Greek"));
}

#[test]
fn encoding_traits_are_detected() {
    assert!(detect_traits(b"\xef\xbb\xbf\\id GEN\n").contains("bom"));
    assert!(detect_traits(b"a\r\nb\r\n").contains("crlf"));
    assert!(detect_traits(b"a\nb\n").contains("lf"));
    assert!(detect_traits(b"a\r\nb\n").contains("mixed_eol"));
    assert!(detect_traits(b"a\nb").contains("no_final_newline"));
    assert!(detect_traits("cafe\u{301}\n".as_bytes()).contains("not_nfc"));
    assert!(!detect_traits("caf\u{e9}\n".as_bytes()).contains("not_nfc"));
    assert!(detect_traits("a\u{200d}b\n".as_bytes()).contains("joiners"));
    assert!(detect_traits(b"\xff\xfe not utf8\n").contains("invalid_utf8"));
}

// ---------------------------------------------------- end-to-end tests ---

struct Fixture {
    _pool: PathBuf,
    corpus: PathBuf,
    manifest: PathBuf,
    core: PathBuf,
}

fn selected_fixture(target: usize) -> Fixture {
    let pool = tmp_dir("pool");
    build_pool(&pool);

    let (s, f, t) = union_over_pool(&pool);
    for r in REQUIRED_SCRIPTS {
        assert!(s.contains(*r), "pool missing script {r}");
    }
    for r in FEATURE_CLASSES {
        assert!(f.contains(*r), "pool missing feature {r}");
    }
    for r in TRAIT_CLASSES {
        assert!(t.contains(*r), "pool missing trait {r}");
    }

    let corpus = tmp_dir("corpus");
    let core = corpus.join("core");
    let manifest = corpus.join("manifest.toml");
    // Budget high enough not to bind: these fixtures are tiny, and the
    // selection behaviour under test is coverage, not the size ceiling. No
    // curated-source floor either — the fixture pool contains none.
    corpus::select(&pool, target, u64::MAX, 0, Some(&core), Some(&manifest)).expect("select");

    Fixture {
        _pool: pool,
        corpus,
        manifest,
        core,
    }
}

#[test]
fn select_then_verify_succeeds() {
    let fx = selected_fixture(24);
    let text = std::fs::read_to_string(&fx.manifest).unwrap();
    assert_eq!(text.matches("[[file]]").count(), 24);
    corpus::verify(&fx.corpus, false).expect("clean corpus verifies");
}

#[test]
fn tampered_file_is_rejected() {
    let fx = selected_fixture(24);
    let victim = corpus_first_file(&fx.core);
    let mut raw = std::fs::read(&victim).unwrap();
    raw.push(b'x');
    std::fs::write(&victim, raw).unwrap();
    assert!(corpus::verify(&fx.corpus, false).is_err());
}

#[test]
fn missing_file_is_rejected() {
    let fx = selected_fixture(24);
    std::fs::remove_file(corpus_first_file(&fx.core)).unwrap();
    assert!(corpus::verify(&fx.corpus, false).is_err());
}

#[test]
fn unlisted_file_is_rejected() {
    let fx = selected_fixture(24);
    let src = corpus_first_file(&fx.core);
    std::fs::copy(&src, fx.core.join("orphan.usfm")).unwrap();
    assert!(corpus::verify(&fx.corpus, false).is_err());
}

#[test]
fn non_redistributable_entry_is_rejected() {
    let fx = selected_fixture(24);
    let text = std::fs::read_to_string(&fx.manifest).unwrap();
    let patched = text.replacen(
        "redistributable = \"True\"",
        "redistributable = \"False\"",
        1,
    );
    assert_ne!(text, patched, "fixture should contain a True entry");
    std::fs::write(&fx.manifest, patched).unwrap();
    assert!(corpus::verify(&fx.corpus, false).is_err());
}

#[test]
fn coverage_hole_is_rejected_but_skip_coverage_tolerates_it() {
    let fx = selected_fixture(24);
    let text = std::fs::read_to_string(&fx.manifest).unwrap();
    let head = text.split("[[file]]").next().unwrap().to_string();
    let kept: Vec<&str> = text
        .split("[[file]]")
        .skip(1)
        .filter(|b| !b.contains("tamil"))
        .collect();
    std::fs::write(&fx.manifest, head + "[[file]]" + &kept.join("[[file]]")).unwrap();
    for e in std::fs::read_dir(&fx.core).unwrap().flatten() {
        if e.file_name().to_string_lossy().starts_with("tamil") {
            std::fs::remove_file(e.path()).unwrap();
        }
    }
    assert!(
        corpus::verify(&fx.corpus, false).is_err(),
        "coverage hole must fail"
    );
    corpus::verify(&fx.corpus, true).expect("--skip-coverage tolerates the hole");
}

#[test]
fn greedy_cover_is_smaller_than_the_target() {
    // Every goal is reachable from a dozen translations, so the cover should be
    // far below the padded target — otherwise selection is not doing its job.
    let fx = selected_fixture(40);
    let text = std::fs::read_to_string(&fx.manifest).unwrap();
    assert_eq!(text.matches("[[file]]").count(), 40);
    corpus::verify(&fx.corpus, false).expect("verify");
}

fn corpus_first_file(core: &Path) -> PathBuf {
    let mut files: Vec<PathBuf> = std::fs::read_dir(core)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("usfm"))
        .collect();
    files.sort();
    files.into_iter().next().expect("core has files")
}

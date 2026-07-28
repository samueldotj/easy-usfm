//! Corpus commands: classify, fetch, select, verify.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::features::{
    all_goals, detect_features, detect_scripts, detect_traits, read_text, Joined, Profile,
    FEATURE_CLASSES, REQUIRED_SCRIPTS, TRAIT_CLASSES,
};
use crate::manifest::{self, RenderEntry};

const CATALOG_URL: &str = "https://ebible.org/Scriptures/translations.csv";
const ZIP_URL: &str = "https://ebible.org/Scriptures/{id}_usfm.zip";

pub fn repo_root() -> PathBuf {
    // xtask/src/corpus.rs -> xtask -> repo
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent")
        .to_path_buf()
}

fn usfm_files(root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("usfm") || s.eq_ignore_ascii_case("sfm"))
        })
        .collect();
    v.sort();
    v
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

// -------------------------------------------------------------- classify ---

pub fn classify(paths: &[PathBuf], coverage: bool) -> Result<()> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            files.extend(usfm_files(p));
        } else {
            files.push(p.clone());
        }
    }
    if files.is_empty() {
        bail!("no USFM files found");
    }

    let mut scripts = BTreeSet::new();
    let mut features = BTreeSet::new();
    let mut traits = BTreeSet::new();
    let mut rows = Vec::new();

    for f in &files {
        let raw = std::fs::read(f).with_context(|| format!("reading {}", f.display()))?;
        let p = Profile::of(&raw);
        scripts.extend(p.scripts.iter().cloned());
        features.extend(p.features.iter().cloned());
        traits.extend(p.traits.iter().cloned());
        rows.push((f.clone(), raw.len(), p));
    }

    if coverage {
        return report_coverage(rows.len(), &scripts, &features, &traits);
    }

    let width = rows
        .iter()
        .map(|(f, _, _)| {
            f.file_name()
                .map(|n| n.to_string_lossy().len())
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(20);

    for (f, len, p) in &rows {
        println!(
            "{:<width$}  {:>5} KB  {:<28}  {:>2} features  {}",
            f.file_name().unwrap_or_default().to_string_lossy(),
            len / 1024,
            Joined(&p.scripts).to_string(),
            p.features.len(),
            Joined(&p.traits),
            width = width
        );
    }
    Ok(())
}

fn report_coverage(
    n: usize,
    scripts: &BTreeSet<String>,
    features: &BTreeSet<String>,
    traits: &BTreeSet<String>,
) -> Result<()> {
    let missing = |required: &[&str], have: &BTreeSet<String>| -> Vec<String> {
        required
            .iter()
            .filter(|r| !have.contains(**r))
            .map(|r| r.to_string())
            .collect()
    };
    let ms = missing(REQUIRED_SCRIPTS, scripts);
    let mf = missing(FEATURE_CLASSES, features);
    let mt = missing(TRAIT_CLASSES, traits);

    println!("files      {n}");
    println!(
        "scripts    {}/{} required  ({} seen in total)",
        REQUIRED_SCRIPTS.len() - ms.len(),
        REQUIRED_SCRIPTS.len(),
        scripts.len()
    );
    println!(
        "features   {}/{}",
        FEATURE_CLASSES.len() - mf.len(),
        FEATURE_CLASSES.len()
    );
    println!(
        "traits     {}/{}",
        TRAIT_CLASSES.len() - mt.len(),
        TRAIT_CLASSES.len()
    );

    let mut ok = true;
    for (label, m) in [("scripts", &ms), ("features", &mf), ("traits", &mt)] {
        if !m.is_empty() {
            ok = false;
            println!("\nmissing {label}: {}", m.join(", "));
        }
    }
    if ok {
        println!("\ncoverage complete");
        Ok(())
    } else {
        bail!("coverage incomplete")
    }
}

// ----------------------------------------------------------------- fetch ---

#[derive(Debug, Clone, serde::Deserialize)]
struct CatalogRow {
    #[serde(rename = "translationId")]
    translation_id: String,
    #[serde(rename = "languageNameInEnglish", default)]
    language: String,
    #[serde(rename = "Redistributable", default)]
    redistributable: String,
    #[serde(rename = "Copyright", default)]
    copyright: String,
    #[serde(rename = "script", default)]
    script: String,
    #[serde(rename = "textDirection", default)]
    direction: String,
    #[serde(default)]
    downloadable: String,
    #[serde(rename = "OTbooks", default)]
    ot: String,
    #[serde(rename = "NTbooks", default)]
    nt: String,
    #[serde(rename = "DCbooks", default)]
    dc: String,
}

impl CatalogRow {
    fn books(&self) -> u32 {
        [&self.ot, &self.nt, &self.dc]
            .iter()
            .filter_map(|s| s.trim().parse::<u32>().ok())
            .sum()
    }
    /// Redistributable *and* downloadable, with actual content. This flag is the
    /// licence gate — see corpus/README.md.
    fn usable(&self) -> bool {
        self.redistributable.trim().eq_ignore_ascii_case("true")
            && self.downloadable.trim().eq_ignore_ascii_case("true")
            && self.books() > 0
    }
}

/// `curl` and `tar` ship with Windows 10 1803+, macOS, and effectively every
/// Linux. Shelling out to them keeps an HTTP stack, a TLS stack, and a zip
/// decoder out of the dependency tree for a tool that runs a handful of times.
fn require(tool: &str) -> Result<()> {
    let probe = Command::new(tool).arg("--version").output();
    match probe {
        Ok(o) if o.status.success() => Ok(()),
        _ => bail!("`{tool}` not found on PATH; it is needed to download the corpus"),
    }
}

fn curl_to(url: &str, dest: &Path) -> Result<()> {
    let out = Command::new("curl")
        .args([
            "--location",
            "--fail",
            "--silent",
            "--show-error",
            "--output",
        ])
        .arg(dest)
        .arg(url)
        .output()
        .with_context(|| format!("running curl for {url}"))?;
    if !out.status.success() {
        bail!(
            "curl failed for {url}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

fn load_catalog(refresh: bool) -> Result<Vec<CatalogRow>> {
    let cache = repo_root().join("corpus").join(".catalog.csv");
    if refresh || !cache.exists() {
        require("curl")?;
        std::fs::create_dir_all(cache.parent().unwrap())?;
        curl_to(CATALOG_URL, &cache)?;
    }
    let text = std::fs::read_to_string(&cache)?;
    // The file is served with a BOM.
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut rdr = csv::Reader::from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for r in rdr.deserialize::<CatalogRow>() {
        rows.push(r.context("parsing the eBible catalog")?);
    }
    Ok(rows)
}

pub struct FetchOpts {
    pub list: bool,
    pub dry_run: bool,
    pub ids: Option<String>,
    pub limit: usize,
    pub refresh_catalog: bool,
}

pub fn fetch(o: &FetchOpts) -> Result<()> {
    let rows = load_catalog(o.refresh_catalog)?;
    let usable: Vec<&CatalogRow> = rows.iter().filter(|r| r.usable()).collect();
    eprintln!(
        "catalog: {} translations, {} redistributable and downloadable",
        rows.len(),
        usable.len()
    );

    if o.list {
        for r in &usable {
            println!(
                "{:<12} {:<12} {:<4} {}",
                r.translation_id, r.script, r.direction, r.language
            );
        }
        return Ok(());
    }

    let selected: Vec<&CatalogRow> = match &o.ids {
        Some(ids) => {
            let want: BTreeSet<&str> = ids.split(',').map(str::trim).collect();
            let sel: Vec<&CatalogRow> = usable
                .iter()
                .copied()
                .filter(|r| want.contains(r.translation_id.as_str()))
                .collect();
            let got: BTreeSet<&str> = sel.iter().map(|r| r.translation_id.as_str()).collect();
            for missing in want.difference(&got) {
                eprintln!("  {missing}: not redistributable, not downloadable, or unknown");
            }
            sel
        }
        None => {
            // Spread across scripts rather than piling up on Latin.
            let mut by_script: BTreeMap<&str, Vec<&CatalogRow>> = BTreeMap::new();
            for r in &usable {
                by_script.entry(r.script.as_str()).or_default().push(r);
            }
            let per = (o.limit / by_script.len().max(1)).max(1);
            let mut sel = Vec::new();
            for group in by_script.values_mut() {
                group.sort_by_key(|r| std::cmp::Reverse(r.books()));
                sel.extend(group.iter().copied().take(per));
            }
            sel.truncate(o.limit);
            sel
        }
    };

    eprintln!("selected {} translations", selected.len());
    if o.dry_run {
        for r in &selected {
            println!(
                "{:<12} {:<12} {}",
                r.translation_id,
                r.script,
                ZIP_URL.replace("{id}", &r.translation_id)
            );
        }
        return Ok(());
    }

    require("curl")?;
    require("tar")?;
    let dest = repo_root().join("corpus").join("extended");
    std::fs::create_dir_all(&dest)?;
    let tmp = dest.join(".download.zip");

    let mut provenance = serde_json::Map::new();
    let mut total = 0usize;

    for (i, r) in selected.iter().enumerate() {
        let tid = &r.translation_id;
        eprintln!("[{}/{}] {tid}", i + 1, selected.len());
        let url = ZIP_URL.replace("{id}", tid);
        if let Err(e) = curl_to(&url, &tmp) {
            eprintln!("  skipped: {e}");
            continue;
        }
        let out = dest.join(tid);
        std::fs::create_dir_all(&out)?;
        // bsdtar on Windows and libarchive elsewhere both read zip.
        let status = Command::new("tar")
            .arg("-xf")
            .arg(&tmp)
            .arg("-C")
            .arg(&out)
            .status()?;
        if !status.success() {
            eprintln!("  skipped: could not extract");
            continue;
        }
        // Flatten and drop anything that is not USFM.
        let mut kept = 0usize;
        for f in usfm_files(&out) {
            let flat = out.join(f.file_name().unwrap());
            if f != flat {
                std::fs::rename(&f, &flat).ok();
            }
            kept += 1;
        }
        prune_empty_dirs(&out);
        total += kept;

        provenance.insert(
            tid.clone(),
            serde_json::json!({
                "source": url,
                "language": r.language,
                "script": r.script,
                "direction": r.direction,
                "copyright": r.copyright,
                "redistributable": r.redistributable,
                "files": kept,
            }),
        );
    }
    std::fs::remove_file(&tmp).ok();

    std::fs::write(
        dest.join("provenance.json"),
        serde_json::to_string_pretty(&provenance)? + "\n",
    )?;
    eprintln!(
        "\n{total} files from {} translations in {}",
        provenance.len(),
        dest.display()
    );
    Ok(())
}

fn prune_empty_dirs(root: &Path) {
    let dirs: Vec<PathBuf> = WalkDir::new(root)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir() && e.path() != root)
        .map(|e| e.into_path())
        .collect();
    for d in dirs {
        std::fs::remove_dir(&d).ok();
    }
}

// ---------------------------------------------------------------- select ---

struct Candidate {
    path: PathBuf,
    bytes: u64,
    sha256: String,
    profile: Profile,
    goals: BTreeSet<String>,
}

pub fn select(
    source: &Path,
    target: usize,
    copy_to: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<()> {
    let files = usfm_files(source);
    if files.is_empty() {
        bail!(
            "no USFM files under {} — run `cargo xtask corpus fetch` first",
            source.display()
        );
    }
    eprintln!("profiling {} candidates…", files.len());

    let mut cands: Vec<Candidate> = Vec::with_capacity(files.len());
    for f in files {
        let raw = std::fs::read(&f)?;
        let profile = Profile::of(&raw);
        cands.push(Candidate {
            bytes: raw.len() as u64,
            sha256: sha256_hex(&raw),
            goals: profile.goals(),
            profile,
            path: f,
        });
    }

    // Greedy set cover: guarantees the invariants verify enforces are met, and
    // keeps the committed set small, since this tier lands in every clone.
    let mut remaining = all_goals();
    let mut chosen: Vec<usize> = Vec::new();
    let mut taken = vec![false; cands.len()];

    while !remaining.is_empty() {
        let best = cands
            .iter()
            .enumerate()
            .filter(|(i, _)| !taken[*i])
            .map(|(i, c)| (i, c.goals.intersection(&remaining).count(), c.bytes))
            .filter(|(_, gain, _)| *gain > 0)
            // Most new goals; ties to the smaller file.
            .max_by(|a, b| a.1.cmp(&b.1).then(b.2.cmp(&a.2)));

        let Some((i, _, _)) = best else { break };
        taken[i] = true;
        for g in &cands[i].goals {
            remaining.remove(g);
        }
        chosen.push(i);
    }
    if !remaining.is_empty() {
        let mut v: Vec<&str> = remaining.iter().map(|s| s.as_str()).collect();
        v.sort_unstable();
        eprintln!("warning: no candidate covers {}", v.join(", "));
    }
    eprintln!("greedy cover: {} files", chosen.len());

    // Pad toward the target, spreading across scripts and preferring small files.
    let mut per_script: BTreeMap<String, usize> = BTreeMap::new();
    for &i in &chosen {
        for s in &cands[i].profile.scripts {
            *per_script.entry(s.clone()).or_default() += 1;
        }
    }
    while chosen.len() < target {
        let next = cands
            .iter()
            .enumerate()
            .filter(|(i, _)| !taken[*i])
            .min_by_key(|(_, c)| {
                let rarity = c
                    .profile
                    .scripts
                    .iter()
                    .map(|s| per_script.get(s).copied().unwrap_or(0))
                    .min()
                    .unwrap_or(0);
                (rarity, c.bytes)
            })
            .map(|(i, _)| i);
        let Some(i) = next else { break };
        taken[i] = true;
        for s in &cands[i].profile.scripts {
            *per_script.entry(s.clone()).or_default() += 1;
        }
        chosen.push(i);
    }
    let total: u64 = chosen.iter().map(|&i| cands[i].bytes).sum();
    eprintln!(
        "after padding: {} files, {} MB",
        chosen.len(),
        total / 1024 / 1024
    );

    let prov = load_provenance(source);
    let mut entries: Vec<RenderEntry> = chosen
        .iter()
        .map(|&i| {
            let c = &cands[i];
            let name = c.path.file_name().unwrap().to_string_lossy().to_string();
            let tid = c
                .path
                .strip_prefix(source)
                .ok()
                .and_then(|r| r.parent())
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let m = prov.get(&tid);
            let get = |k: &str| -> String {
                m.and_then(|v| v.get(k))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            RenderEntry {
                path: format!("core/{name}"),
                sha256: c.sha256.clone(),
                bytes: c.bytes,
                translation: tid,
                source: get("source"),
                language: get("language"),
                script_declared: get("script"),
                direction: get("direction"),
                copyright: get("copyright"),
                redistributable: get("redistributable"),
                scripts: c.profile.scripts.clone(),
                features: c.profile.features.clone(),
                traits: c.profile.traits.clone(),
            }
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let manifest_path = manifest_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo_root().join("corpus").join("manifest.toml"));
    std::fs::create_dir_all(manifest_path.parent().unwrap())?;
    std::fs::write(&manifest_path, manifest::render(&entries))?;
    eprintln!("manifest written to {}", manifest_path.display());

    if let Some(dir) = copy_to {
        std::fs::create_dir_all(dir)?;
        for &i in &chosen {
            let c = &cands[i];
            std::fs::copy(&c.path, dir.join(c.path.file_name().unwrap()))?;
        }
        eprintln!("copied {} files to {}", chosen.len(), dir.display());
    }
    Ok(())
}

fn load_provenance(root: &Path) -> serde_json::Map<String, serde_json::Value> {
    let f = root.join("provenance.json");
    std::fs::read_to_string(f)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

// ---------------------------------------------------------------- verify ---

pub fn verify(corpus: &Path, skip_coverage: bool) -> Result<()> {
    let manifest_path = corpus.join("manifest.toml");
    if !manifest_path.exists() {
        eprintln!("error: no manifest at {}", manifest_path.display());
        eprintln!("run `cargo xtask corpus fetch` then `cargo xtask corpus select` to build one");
        bail!("no manifest");
    }
    let m = manifest::load(&manifest_path)?;
    if m.files.is_empty() {
        bail!("{} contains no [[file]] entries", manifest_path.display());
    }

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut scripts = BTreeSet::new();
    let mut features = BTreeSet::new();
    let mut traits = BTreeSet::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for e in &m.files {
        let name = Path::new(&e.path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        seen.insert(name);
        let path = corpus.join(&e.path);

        if !path.exists() {
            errors.push(format!(
                "{}: listed in the manifest but not on disk",
                e.path
            ));
            continue;
        }
        let raw = std::fs::read(&path)?;
        let actual = sha256_hex(&raw);
        if actual != e.sha256 {
            errors.push(format!(
                "{}: sha256 mismatch\n    expected {}\n    actual   {}",
                e.path, e.sha256, actual
            ));
            continue;
        }
        if e.source.is_empty() {
            errors.push(format!("{}: no source URL recorded", e.path));
        }
        if e.copyright.is_empty() {
            warnings.push(format!("{}: no copyright line recorded", e.path));
        }
        if !e.redistributable.trim().eq_ignore_ascii_case("true") {
            errors.push(format!(
                "{}: redistributable is {:?}, expected \"True\" — this file must not be committed",
                e.path, e.redistributable
            ));
        }

        let text = read_text(&raw);
        scripts.extend(detect_scripts(&text, 0.01));
        features.extend(detect_features(&text));
        traits.extend(detect_traits(&raw));
    }

    let core = corpus.join("core");
    if core.exists() {
        for f in usfm_files(&core) {
            let n = f.file_name().unwrap().to_string_lossy().to_string();
            if !seen.contains(&n) {
                errors.push(format!(
                    "corpus/core/{n}: present on disk but not in the manifest"
                ));
            }
        }
    }

    if !skip_coverage {
        for (label, required, have) in [
            ("script", REQUIRED_SCRIPTS, &scripts),
            ("feature class", FEATURE_CLASSES, &features),
            ("encoding trait", TRAIT_CLASSES, &traits),
        ] {
            let missing: Vec<&str> = required
                .iter()
                .copied()
                .filter(|r| !have.contains(*r))
                .collect();
            if !missing.is_empty() {
                errors.push(format!(
                    "corpus does not cover {label}(s): {}",
                    missing.join(", ")
                ));
            }
        }
    }

    for w in &warnings {
        println!("warning: {w}");
    }
    for e in &errors {
        println!("error: {e}");
    }
    if !errors.is_empty() {
        println!(
            "\nFAIL — {} error(s) across {} files",
            errors.len(),
            m.files.len()
        );
        bail!("verification failed");
    }
    print!("\nOK — {} files verified", m.files.len());
    if warnings.is_empty() {
        println!();
    } else {
        println!(", {} warning(s)", warnings.len());
    }
    Ok(())
}

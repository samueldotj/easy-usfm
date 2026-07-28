//! Curated corpus sources held in Git repositories.
//!
//! eBible.org supplies breadth — hundreds of translations behind one catalogue
//! with a machine-readable redistribution flag. It does not supply everything:
//! some translations are published only as a repository, and for the scripts
//! that matter most to this project the repository version is often the better
//! text.
//!
//! These sources are therefore listed by hand rather than discovered, and each
//! carries its licence basis with it. That is the same standard `verify`
//! applies to eBible entries — the difference is that here a human confirmed
//! the terms rather than a catalogue column, so the evidence is recorded in
//! the table below and ends up in the manifest.
//!
//! **Pinned to a commit, not a branch.** A branch moves, and a corpus that
//! silently changes underneath a failing test turns every debugging session
//! into an archaeology exercise. Updating a pin is a deliberate edit here.

use std::path::Path;

use anyhow::{Context, Result};

/// A repository that publishes USFM, and the basis for redistributing it.
pub struct Source {
    /// Directory name under `corpus/extended/`, and the `translation` field in
    /// the manifest.
    pub id: &'static str,
    /// `owner/name` on GitHub.
    pub repo: &'static str,
    /// Commit SHA. Not a branch — see the module note.
    pub commit: &'static str,
    /// Path within the repository holding the USFM.
    pub subdir: &'static str,
    pub language: &'static str,
    /// The script as published, in the vocabulary the eBible catalogue uses,
    /// so both kinds of source produce comparable manifest entries.
    pub script: &'static str,
    pub direction: &'static str,
    /// The copyright or licence line, recorded verbatim so the basis for
    /// committing the file is auditable without re-reading anything.
    pub copyright: &'static str,
    /// Where that line came from.
    pub licence_url: &'static str,
}

impl Source {
    /// The tarball for the pinned commit.
    ///
    /// `codeload` serves these directly; the alternative is the archive
    /// redirect on `github.com`, which costs an extra round trip.
    pub fn tarball_url(&self) -> String {
        format!(
            "https://codeload.github.com/{}/tar.gz/{}",
            self.repo, self.commit
        )
    }

    /// What lands in the manifest's `source` field: a permalink to the exact
    /// tree the file came from, not a branch URL that will drift.
    pub fn permalink(&self) -> String {
        format!(
            "https://github.com/{}/tree/{}/{}",
            self.repo, self.commit, self.subdir
        )
    }
}

/// The curated set.
///
/// Between them these cover Tamil, Devanagari, and Latin. They do **not**
/// cover the other nine scripts ARCHITECTURE §12.4 requires — in particular
/// nothing here is right-to-left, and nothing here is a script without word
/// spacing. eBible remains the source for those, which is why `fetch` runs
/// both by default.
pub const SOURCES: &[Source] = &[
    Source {
        id: "freebiblesindia-tamil",
        repo: "FreeBiblesIndia/Tamil_Bible",
        commit: "c4f4695592040f2dde010a9e1192d36690376342",
        subdir: "usfm",
        language: "Tamil",
        script: "Tamil",
        direction: "ltr",
        copyright: "Creative Commons Attribution-ShareAlike 4.0 International. \
                    Original work available at http://www.freebiblesindia.in",
        licence_url: "https://github.com/FreeBiblesIndia/Tamil_Bible/blob/master/LICENSE.md",
    },
    Source {
        id: "freebiblesindia-hindi",
        repo: "FreeBiblesIndia/Hindi_Bible",
        commit: "0bc468f34cbfb77fefeb2685616e91fbd7d55101",
        subdir: "usfm",
        language: "Hindi",
        script: "Devanagari",
        direction: "ltr",
        copyright: "Creative Commons Attribution-ShareAlike 4.0 International. \
                    Original work available at http://www.freebiblesindia.in",
        licence_url: "https://github.com/FreeBiblesIndia/Hindi_Bible/blob/master/LICENSE.md",
    },
    Source {
        id: "bsb",
        repo: "dharmatech/bsb-usfm",
        commit: "5186f55fb2ffdcd63a94aa4e6f9073f06606f72c",
        subdir: "usfm",
        language: "English",
        script: "Latin",
        direction: "ltr",
        // The repository itself carries no LICENSE file; the claim is made in
        // its README and rests on the publisher's own terms, which is what the
        // licence_url points at rather than the mirror.
        copyright: "Public domain. The Berean Standard Bible, berean.bible",
        licence_url: "https://berean.bible/licensing.htm",
    },
];

/// Downloads each source into `dest/<id>/`, flattened to USFM files only.
///
/// Returns one provenance entry per source, in the shape `select` expects, so
/// repository-sourced files and eBible files produce identical manifest rows.
pub fn fetch_all(
    dest: &Path,
    dry_run: bool,
    only: Option<&str>,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let selected: Vec<&Source> = SOURCES
        .iter()
        .filter(|source| only.is_none_or(|want| want.split(',').any(|w| w.trim() == source.id)))
        .collect();

    let mut provenance = serde_json::Map::new();

    for (index, source) in selected.iter().enumerate() {
        eprintln!(
            "[{}/{}] {} ({})",
            index + 1,
            selected.len(),
            source.id,
            source.repo
        );

        if dry_run {
            println!(
                "{:<24} {:<12} {}",
                source.id,
                source.script,
                source.tarball_url()
            );
            continue;
        }

        let kept = match fetch_one(source, dest) {
            Ok(kept) => kept,
            Err(error) => {
                eprintln!("  skipped: {error}");
                continue;
            }
        };
        eprintln!("  {kept} files");

        provenance.insert(
            source.id.to_string(),
            serde_json::json!({
                "source": source.permalink(),
                "language": source.language,
                "script": source.script,
                "direction": source.direction,
                "copyright": source.copyright,
                "licence_url": source.licence_url,
                // Confirmed by reading the licence, not by a catalogue column.
                // verify treats both kinds the same.
                "redistributable": "True",
                "files": kept,
            }),
        );
    }

    Ok(provenance)
}

fn fetch_one(source: &Source, dest: &Path) -> Result<usize> {
    let out = dest.join(source.id);
    // A stale partial extraction would be silently included in the pool.
    if out.exists() {
        std::fs::remove_dir_all(&out).ok();
    }
    std::fs::create_dir_all(&out)?;

    let staging = dest.join(format!(".{}.staging", source.id));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).ok();
    }
    std::fs::create_dir_all(&staging)?;

    let archive = dest.join(format!(".{}.tar.gz", source.id));
    let result = extract_into(source, &archive, &staging, &out);

    std::fs::remove_file(&archive).ok();
    std::fs::remove_dir_all(&staging).ok();

    result
}

fn extract_into(source: &Source, archive: &Path, staging: &Path, out: &Path) -> Result<usize> {
    crate::corpus::curl_to(&source.tarball_url(), archive)?;

    let status = std::process::Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(staging)
        .status()
        .context("running tar")?;
    anyhow::ensure!(status.success(), "could not extract the archive");

    // GitHub wraps the tree in a single `<name>-<commit>` directory, so the
    // subdir we want is one level below whatever that turned out to be.
    let root = std::fs::read_dir(staging)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.is_dir())
        .context("the archive contained no directory")?;

    let wanted = root.join(source.subdir);
    anyhow::ensure!(
        wanted.is_dir(),
        "{} does not exist in the archive",
        source.subdir
    );

    // Only USFM is copied across, which drops the .xml, .sty, .vrs, and .ldml
    // files some of these repositories keep alongside the text.
    let mut kept = 0;
    for file in crate::corpus::usfm_files(&wanted) {
        let name = file.file_name().context("a file with no name")?;
        std::fs::copy(&file, out.join(name))?;
        kept += 1;
    }

    anyhow::ensure!(kept > 0, "no USFM files under {}", source.subdir);
    Ok(kept)
}

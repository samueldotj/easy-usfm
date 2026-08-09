//! Find, across both spellings and both modes — P2.11.
//!
//! UNICODE §4 states the failure this exists to prevent: "a search for a word
//! gets zero hits because the keyboard produced NFC and the file is NFD, with
//! the word visibly on screen. That is the most infuriating bug this class of
//! application can have."

use usfm_core::Session;

/// "café" composed, and decomposed. Different bytes, identical on screen.
const NFC: &str = "caf\u{e9}";
const NFD: &str = "cafe\u{301}";

fn document(word: &str) -> String {
    format!("\\id GEN\n\\c 1\n\\p\n\\v 1 a {word} here\n\\v 2 and {word} again\n")
}

#[test]
fn an_nfc_query_finds_nfd_text() {
    let source = document(NFD);
    let session = Session::new(source.clone());
    let hits = session.find(NFC);

    assert_eq!(hits.len(), 2, "the NFC query found no NFD text");
    for hit in &hits {
        assert_eq!(hit.slice(&source), Some(NFD));
    }
}

#[test]
fn an_nfd_query_finds_nfc_text() {
    let source = document(NFC);
    let session = Session::new(source.clone());
    let hits = session.find(NFD);

    assert_eq!(hits.len(), 2);
    for hit in &hits {
        assert_eq!(hit.slice(&source), Some(NFC));
    }
}

#[test]
fn the_exact_toggle_tells_the_two_spellings_apart() {
    // The job the default cannot do: finding out which spelling a file uses.
    let source = format!("\\id GEN\n\\c 1\n\\p\n\\v 1 {NFC} and {NFD}\n");
    let session = Session::new(source.clone());

    // Normalized: both, because they are the same word.
    assert_eq!(session.find(NFC).len(), 2);
    assert_eq!(session.find(NFD).len(), 2);

    // Exact: one each, and each is the spelling asked for.
    let composed = session.find_exact(NFC);
    let decomposed = session.find_exact(NFD);
    assert_eq!(composed.len(), 1);
    assert_eq!(decomposed.len(), 1);
    assert_eq!(composed[0].slice(&source), Some(NFC));
    assert_eq!(decomposed[0].slice(&source), Some(NFD));
}

#[test]
fn the_buffer_is_never_normalized_by_searching() {
    // The whole point of normalizing for comparison only. A file in NFD must
    // still be in NFD after someone has searched it.
    let source = document(NFD);
    let session = Session::new(source.clone());

    let _ = session.find(NFC);
    let _ = session.find_exact(NFD);

    assert_eq!(session.source(), source);
}

#[test]
fn matches_land_on_whole_characters_in_complex_scripts() {
    // A hit covering half a conjunct would put the selection mid-character,
    // which is the class of failure UNICODE §3 exists to prevent.
    for word in [
        "\u{B95}\u{BCD}\u{BB7}",
        "\u{915}\u{94D}\u{937}",
        "\u{5D1}\u{5BC}",
    ] {
        let source = document(word);
        let session = Session::new(source.clone());
        let hits = session.find(word);

        assert!(!hits.is_empty(), "{word:?} did not find itself");
        for hit in hits {
            assert!(
                source.is_char_boundary(hit.start) && source.is_char_boundary(hit.end),
                "{word:?} matched mid-character at {hit:?}"
            );
            assert!(hit.slice(&source).is_some());
        }
    }
}

#[test]
fn overlapping_occurrences_are_not_reported_twice() {
    let source = "\\id GEN\n\\c 1\n\\p\n\\v 1 aaaa\n".to_string();
    let session = Session::new(source);

    // "aa" occurs at 0, 1, 2 by overlap; "find the next one" means three
    // separate hits is wrong and two is right.
    assert_eq!(session.find_exact("aa").len(), 2);
    assert_eq!(session.find("aa").len(), 2);
}

#[test]
fn an_empty_query_matches_nothing() {
    let session = Session::new(document("word"));
    assert!(session.find("").is_empty());
    assert!(session.find_exact("").is_empty());
}

#[test]
fn a_marker_can_be_searched_for_like_any_other_text() {
    // Find operates on the source, which is what the editor shows.
    let session = Session::new(document("word"));
    assert_eq!(session.find_exact("\\v ").len(), 2);
}

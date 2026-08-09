//! The envelope, over arbitrary bytes — P4.5.
//!
//! FILE-FIDELITY §1's guarantee is byte-for-byte preservation of a file nobody
//! has edited, and it is the one guarantee whose failure is silent: the file
//! saves, the editor says so, and a diff touching every line appears in
//! somebody's repository a week later. A fuzzer is the right tool because the
//! interesting inputs are precisely the ones no author would write — a lone
//! carriage return before a byte-order mark, a file that is nothing but
//! terminators.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(loaded) = usfm_core::FileFidelity::capture(data) else {
        // Refusing is a valid outcome; a lossy decode is not.
        return;
    };

    assert_eq!(
        loaded.fidelity.serialize(&loaded.text),
        data,
        "an unedited file did not survive a round trip"
    );
});

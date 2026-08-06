//! The update check — PRODUCT §11, SECURITY §6, P6.3.
//!
//! "Signed updater against a static endpoint, **opt-in on first run**, with the
//! prompt stating it is the application's only network request. Never installs
//! without consent or during an unsaved edit. An offline build variant with the
//! updater compiled out exists for restricted deployments."
//!
//! SECURITY §6 puts it more sharply: "Network activity is exactly one kind of
//! request, ever."
//!
//! # The offline variant is a compile-out, not a setting
//!
//! `--no-default-features` removes this module's ability to make a request at
//! all: the code that would call out is behind `#[cfg(feature = "updater")]`,
//! so in the offline build there is nothing to enable, nothing to misconfigure,
//! and nothing an attacker could turn back on. A setting defaulting to off
//! would be a promise; a missing code path is a fact, and "restricted
//! deployments" are exactly the places that need the difference.
//!
//! That is also why `check` returns a distinct outcome for the offline build
//! rather than pretending the check failed. An interface that says "could not
//! reach the update server" in a build that has no update server is telling the
//! user something false about their own machine.
//!
//! # Opt-in means never having asked counts as no
//!
//! The stored setting is a tri-state — asked and allowed, asked and refused,
//! never asked — because the first and third are not the same. Storing a bare
//! boolean makes "never asked" indistinguishable from "asked and refused", and
//! the honest default for a network request nobody has consented to is not to
//! make it.

use serde::{Deserialize, Serialize};

/// Whether the user has been asked, and what they said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Consent {
    /// Never asked. Not the same as refused, and not a reason to check.
    Unasked,
    Allowed,
    Refused,
}

/// What a check produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Outcome {
    /// This build cannot check at all. Said plainly rather than as a failure.
    CompiledOut,
    /// The user has not consented, so nothing was sent.
    NotPermitted { consent: Consent },
    /// Checked, and this is the newest version.
    UpToDate,
    /// Checked, and there is a newer one.
    Available { version: String, notes: String },
    /// The request was made and did not succeed.
    Failed { reason: String },
}

/// Whether this build can make a network request at all.
///
/// Reported to the interface so it can hide the setting entirely rather than
/// offering a switch that does nothing.
#[tauri::command]
pub fn updates_possible() -> bool {
    cfg!(feature = "updater")
}

/// Checks for an update, if this build can and the user has agreed.
///
/// The consent is passed in rather than read here: it lives in the interface's
/// settings with everything else the user has chosen (PRODUCT §12), and a
/// second copy in the shell would be a second thing that could disagree about
/// whether a request is allowed.
#[tauri::command]
pub async fn check_for_update(consent: Consent) -> Outcome {
    if !cfg!(feature = "updater") {
        return Outcome::CompiledOut;
    }
    if consent != Consent::Allowed {
        return Outcome::NotPermitted { consent };
    }

    check_now().await
}

#[cfg(feature = "updater")]
async fn check_now() -> Outcome {
    // The endpoint and the signing key belong to P6.2, which is lead-time
    // bound: a signed updater needs a certificate and a published endpoint,
    // and neither can be invented here. What this establishes is the gate --
    // that nothing reaches the network without consent, and that the offline
    // build cannot reach it at all.
    Outcome::Failed {
        reason: "No update endpoint is configured for this build.".to_string(),
    }
}

#[cfg(not(feature = "updater"))]
async fn check_now() -> Outcome {
    // Unreachable: `check_for_update` returns `CompiledOut` first. Present so
    // both arms compile, and so the offline build contains no call site.
    Outcome::CompiledOut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_having_asked_is_not_consent() {
        // The whole point of the tri-state. A bare boolean makes this
        // indistinguishable from a refusal, and the honest default for a
        // request nobody has agreed to is not to make it.
        assert_ne!(Consent::Unasked, Consent::Allowed);
        assert_ne!(Consent::Refused, Consent::Allowed);
    }

    #[tokio::test]
    async fn nothing_is_sent_without_consent() {
        for consent in [Consent::Unasked, Consent::Refused] {
            let outcome = check_for_update(consent).await;
            assert!(
                matches!(
                    outcome,
                    Outcome::NotPermitted { .. } | Outcome::CompiledOut
                ),
                "consent {consent:?} produced {outcome:?}"
            );
        }
    }

    #[tokio::test]
    #[cfg(not(feature = "updater"))]
    async fn the_offline_build_says_so_rather_than_failing() {
        // "Could not reach the update server" in a build with no update server
        // tells the user something false about their own machine.
        assert_eq!(check_for_update(Consent::Allowed).await, Outcome::CompiledOut);
        assert!(!updates_possible());
    }
}

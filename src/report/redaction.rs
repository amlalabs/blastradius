//! Layer 2 — final defensive sweep (§4.3). Probes already collect metadata only
//! (Layer 1); this is defense-in-depth run over serialized output before render.

use regex::Regex;
use std::sync::OnceLock;

/// Patterns for known secret shapes (§4.3). Conservative — presence of any of
/// these in rendered output is a bug, so the canary self-test (§4.4) asserts
/// none survive.
fn secret_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let raw = [
            r"ghp_[A-Za-z0-9]{20,}",
            r"github_pat_[A-Za-z0-9_]{20,}",
            r"\bsk-[A-Za-z0-9_\-]{16,}",
            r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
            r"\bxox[bp]-[A-Za-z0-9\-]{10,}",
            r"\bnpm_[A-Za-z0-9]{20,}",
            r"\bglpat-[A-Za-z0-9_\-]{16,}",
            // JWT-shaped: three base64url segments separated by dots.
            r"\beyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\b",
            // PEM private key blocks.
            r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
            // user:pass@host credential URLs.
            r"[A-Za-z][A-Za-z0-9+.-]*://[^/\s:@]+:[^/\s@]+@",
            // Scheme-less proxy-style credentials: user:pass@host.
            r"(?:^|[\s=])[^/\s:@=]+:[^/\s@]+@[^/\s]+",
        ];
        raw.iter().map(|p| Regex::new(p).unwrap()).collect()
    })
}

/// Whether `text` contains anything matching a known secret shape.
pub fn contains_secret_shaped(text: &str) -> bool {
    secret_patterns().iter().any(|re| re.is_match(text))
}

/// Run the final defensive sweep over rendered output. Returns the text
/// unchanged when clean; otherwise replaces each match with `[REDACTED]`.
///
/// In normal operation Layer 1 guarantees no secrets reach here, so this should
/// be a no-op. If it ever fires, that's a Layer-1 bug — surfaced loudly.
pub fn sweep(text: &str) -> String {
    let mut out = text.to_string();
    for re in secret_patterns() {
        if re.is_match(&out) {
            out = re.replace_all(&out, "[REDACTED]").to_string();
        }
    }
    crate::report::sanitize::block(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_shapes() {
        assert!(contains_secret_shaped(
            "token ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"
        ));
        assert!(contains_secret_shaped("AKIAIOSFODNN7EXAMPLE"));
        assert!(contains_secret_shaped(
            "url https://user:hunter2@example.com/x"
        ));
        assert!(contains_secret_shaped(
            "HTTP_PROXY=user:hunter2@proxy.example:8080"
        ));
        assert!(contains_secret_shaped("-----BEGIN RSA PRIVATE KEY-----"));
    }

    #[test]
    fn leaves_clean_text() {
        let clean = "GITHUB_TOKEN present in env — 40 chars";
        assert!(!contains_secret_shaped(clean));
        assert_eq!(sweep(clean), clean);
    }

    #[test]
    fn sweep_redacts() {
        let dirty = "leak ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345 here";
        let cleaned = sweep(dirty);
        assert!(!contains_secret_shaped(&cleaned));
        assert!(cleaned.contains("[REDACTED]"));
    }
}

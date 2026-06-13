//! §extra — browser session & saved-credential stores.
//!
//! The most commonly-overlooked ambient surface: an agent running as you can read
//! your browsers' **cookie jars** (live session tokens for every logged-in site)
//! and **saved-password databases**. Reading a cookie DB is session hijack that
//! bypasses passwords AND MFA; the Login Data DB holds saved credentials. People
//! lock down `~/.aws` and never think about `~/.config/google-chrome`.
//!
//! READ-ONLY and value-free: this probe only checks for the PRESENCE of the store
//! files and counts how many browser profiles have them. It never opens a cookie
//! jar or password DB — no SQLite is parsed, no value is read. (On some platforms
//! the values are OS-encrypted, but the agent can typically also reach the
//! decryption key; presence is the honest reachability signal.)

use serde_json::json;

use crate::context::Context;
use crate::finding::{Finding, FindingClass, FindingScope};
use crate::runner::Probe;
use crate::severity::{Confidence, Severity};
use crate::util::paths::shorten;

pub struct BrowserStoresProbe;

/// A browser family: a root dir under home and the per-profile store file names.
struct Browser {
    name: &'static str,
    /// Candidate root dirs (relative to home) — we try each platform layout.
    roots: &'static [&'static str],
    /// Store files to look for within each profile dir, tagged by kind.
    cookie_files: &'static [&'static str],
    login_files: &'static [&'static str],
    /// Chromium-style nests stores under per-profile subdirs (Default, Profile 1);
    /// Firefox nests under `<root>/<profile>.default*`. We walk one level down.
    nested: bool,
}

const BROWSERS: &[Browser] = &[
    Browser {
        name: "Chrome",
        roots: &[
            ".config/google-chrome",
            "Library/Application Support/Google/Chrome",
            "AppData/Local/Google/Chrome/User Data",
        ],
        cookie_files: &["Cookies", "Network/Cookies"],
        login_files: &["Login Data"],
        nested: true,
    },
    Browser {
        name: "Chromium",
        roots: &[".config/chromium", "Library/Application Support/Chromium"],
        cookie_files: &["Cookies", "Network/Cookies"],
        login_files: &["Login Data"],
        nested: true,
    },
    Browser {
        name: "Brave",
        roots: &[
            ".config/BraveSoftware/Brave-Browser",
            "Library/Application Support/BraveSoftware/Brave-Browser",
        ],
        cookie_files: &["Cookies", "Network/Cookies"],
        login_files: &["Login Data"],
        nested: true,
    },
    Browser {
        name: "Edge",
        roots: &[
            ".config/microsoft-edge",
            "Library/Application Support/Microsoft Edge",
        ],
        cookie_files: &["Cookies", "Network/Cookies"],
        login_files: &["Login Data"],
        nested: true,
    },
    Browser {
        name: "Firefox",
        roots: &[
            ".mozilla/firefox",
            "Library/Application Support/Firefox/Profiles",
            "snap/firefox/common/.mozilla/firefox",
        ],
        cookie_files: &["cookies.sqlite"],
        login_files: &["logins.json", "key4.db"],
        nested: true,
    },
];

impl Probe for BrowserStoresProbe {
    fn id(&self) -> &'static str {
        "browser.session_stores"
    }
    fn class(&self) -> FindingClass {
        FindingClass::Credentials
    }

    fn run(&self, ctx: &Context) -> anyhow::Result<Vec<Finding>> {
        let home = match &ctx.home {
            Some(h) => h.clone(),
            None => {
                return Ok(vec![Finding::new(
                    self.id(),
                    self.class(),
                    FindingScope::Ambient,
                    "browser stores not checked (home unknown)",
                    Severity::Info,
                    Confidence::Unknown,
                )])
            }
        };

        let mut profiles_with_cookies = 0usize;
        let mut profiles_with_logins = 0usize;
        let mut browsers_json: Vec<serde_json::Value> = Vec::new();

        for b in BROWSERS {
            let mut b_cookie = 0usize;
            let mut b_login = 0usize;
            for root_rel in b.roots {
                let root = home.join(root_rel);
                if !root.is_dir() {
                    continue;
                }
                // Profile dirs: the root itself plus one level of subdirs.
                let mut profile_dirs = vec![root.clone()];
                if b.nested {
                    if let Ok(entries) = std::fs::read_dir(&root) {
                        for e in entries.flatten() {
                            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                profile_dirs.push(e.path());
                            }
                        }
                    }
                }
                for dir in &profile_dirs {
                    if b.cookie_files.iter().any(|f| dir.join(f).is_file()) {
                        b_cookie += 1;
                    }
                    if b.login_files.iter().any(|f| dir.join(f).is_file()) {
                        b_login += 1;
                    }
                }
            }
            if b_cookie > 0 || b_login > 0 {
                profiles_with_cookies += b_cookie;
                profiles_with_logins += b_login;
                browsers_json.push(json!({
                    "browser": b.name,
                    "profiles_with_cookie_store": b_cookie,
                    "profiles_with_login_store": b_login,
                }));
            }
        }

        let any = profiles_with_cookies > 0 || profiles_with_logins > 0;
        let severity = if any { Severity::Exposed } else { Severity::Info };

        let title = if any {
            "browser cookie jars / saved passwords reachable"
        } else {
            "no browser session stores reachable"
        };
        let summary = if any {
            format!(
                "{} profile(s) with a cookie jar (live session tokens) and {} with a saved-password store across {} browser(s) — reading them is session hijack past password+MFA",
                profiles_with_cookies,
                profiles_with_logins,
                browsers_json.len()
            )
        } else {
            "no Chromium/Firefox cookie or login-data stores found in home".to_string()
        };

        Ok(vec![Finding::new(
            self.id(),
            self.class(),
            FindingScope::Ambient,
            title,
            severity,
            Confidence::Confirmed,
        )
        .summary(summary)
        .evidence(json!({
            "profiles_with_cookie_store": profiles_with_cookies,
            "profiles_with_login_store": profiles_with_logins,
            "browsers": browsers_json,
            "home": shorten(&home, Some(&home)),
            "note": "Presence/counts only — no cookie jar or password DB is ever opened. Cookie values may be OS-encrypted, but the agent can typically reach the decryption key too.",
        }))
        .remediation(&[
            "Filesystem-isolate browser profile dirs (~/.config/google-chrome, ~/.mozilla, etc.) from agents — a cookie jar is a bearer credential for every logged-in site.",
            "Run agents under a separate OS user/profile that has never logged into a browser.",
        ])])
    }
}

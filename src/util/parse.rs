//! Small parsers shared across probes — all value-free by construction.

/// Parse INI-style section names from text (e.g. AWS credentials/config).
/// `[prod]` → `prod`; `[profile staging]` → `staging`. Values are ignored.
pub fn ini_section_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            let inner = inner.trim();
            // AWS config uses `[profile name]`; credentials uses `[name]`.
            let name = inner
                .strip_prefix("profile ")
                .map(|s| s.trim())
                .unwrap_or(inner);
            if !name.is_empty() {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Extract dotenv-style KEY names from text (§12.6). Never returns values.
/// Matches `^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=`.
pub fn dotenv_keys(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let rest = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let rest = rest.trim_start();
        // Find the key portion up to '='.
        if let Some(eq) = rest.find('=') {
            let key = rest[..eq].trim();
            if is_ident(key) {
                out.push(key.to_string());
            }
        }
    }
    out
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Redact `user:pass@host` userinfo from a URL-ish string, keeping structure.
/// `https://user:tok@github.com/o/r` → `https://***@github.com/o/r`.
pub fn redact_url_userinfo(url: &str) -> String {
    // Operate on the authority section after `scheme://`.
    if let Some(scheme_end) = url.find("://") {
        let (scheme, rest) = url.split_at(scheme_end + 3);
        if let Some(at) = rest.find('@') {
            let after = &rest[at + 1..];
            return format!("{scheme}***@{after}");
        }
    }

    // Proxy variables are often configured without a scheme:
    // `HTTP_PROXY=user:pass@proxy:8080`. Redact those credentials, but avoid
    // treating scp-like git remotes (`git@github.com:org/repo`) as userinfo.
    let authority_end = url
        .find(|c| matches!(c, '/' | '?' | '#'))
        .unwrap_or(url.len());
    let authority = &url[..authority_end];
    if let Some(at) = authority.find('@') {
        let before = &authority[..at];
        let credential_start = before.rfind('=').map(|i| i + 1).unwrap_or(0);
        let userinfo = &before[credential_start..];
        if userinfo.contains(':') {
            let prefix = &url[..credential_start];
            let after = &url[at + 1..];
            return format!("{prefix}***@{after}");
        }
    }
    url.to_string()
}

/// Best-effort extraction of `{ host, protocol }` from a git remote URL (§12.10).
pub fn git_remote_host_protocol(url: &str) -> (Option<String>, Option<String>) {
    let url = url.trim();
    // scp-like: git@github.com:owner/repo.git
    if !url.contains("://") {
        if let Some(at) = url.find('@') {
            let after = &url[at + 1..];
            let host = after.split(':').next().unwrap_or("").to_string();
            if !host.is_empty() {
                return (Some(host), Some("ssh".to_string()));
            }
        }
        return (None, None);
    }
    // scheme://[user@]host[:port]/...
    let scheme = url.split("://").next().unwrap_or("").to_string();
    let protocol = match scheme.as_str() {
        "https" | "http" => "https".to_string(),
        "ssh" => "ssh".to_string(),
        "git" => "git".to_string(),
        other => other.to_string(),
    };
    let after_scheme = &url[scheme.len() + 3..];
    let authority = after_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("");
    let host = authority.split(':').next().unwrap_or("").to_string();
    let host = if host.is_empty() { None } else { Some(host) };
    (host, Some(protocol))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_sections() {
        let t = "[default]\nx=1\n[profile staging]\n[prod]\n";
        assert_eq!(ini_section_names(t), vec!["default", "staging", "prod"]);
    }

    #[test]
    fn dotenv_key_extraction() {
        let t = "# comment\nexport FOO=bar\nBAZ = qux\n  QUX=1\nnotvalid\n1BAD=x\n";
        assert_eq!(dotenv_keys(t), vec!["FOO", "BAZ", "QUX"]);
    }

    #[test]
    fn url_redaction() {
        assert_eq!(
            redact_url_userinfo("https://user:tok@github.com/o/r"),
            "https://***@github.com/o/r"
        );
        assert_eq!(
            redact_url_userinfo("HTTP_PROXY=user:pass@proxy:8080"),
            "HTTP_PROXY=***@proxy:8080"
        );
        assert_eq!(
            redact_url_userinfo("user:pass@proxy:8080"),
            "***@proxy:8080"
        );
        assert_eq!(
            redact_url_userinfo("git@github.com:o/r.git"),
            "git@github.com:o/r.git"
        );
        assert_eq!(
            redact_url_userinfo("https://github.com/o/r"),
            "https://github.com/o/r"
        );
    }

    #[test]
    fn remote_parsing() {
        assert_eq!(
            git_remote_host_protocol("git@github.com:o/r.git"),
            (Some("github.com".into()), Some("ssh".into()))
        );
        assert_eq!(
            git_remote_host_protocol("https://github.com/o/r.git"),
            (Some("github.com".into()), Some("https".into()))
        );
        assert_eq!(
            git_remote_host_protocol("ssh://git@example.com:22/o/r"),
            (Some("example.com".into()), Some("ssh".into()))
        );
    }
}

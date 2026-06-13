//! Network target parsing helpers.

use std::net::{IpAddr, Ipv6Addr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPort {
    pub host: String,
    pub port: u16,
}

impl HostPort {
    pub fn target(&self) -> String {
        if self.host.parse::<Ipv6Addr>().is_ok() {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// Parse and validate a user-provided `HOST:PORT` target.
pub fn parse_host_port_target(target: &str) -> Result<HostPort, &'static str> {
    if target.is_empty() {
        return Err("target must be HOST:PORT");
    }
    if target.trim() != target
        || target
            .chars()
            .any(|c| c.is_ascii_control() || c.is_whitespace())
    {
        return Err("target must not contain whitespace");
    }
    if target.contains("://") {
        return Err("target must be HOST:PORT without a URL scheme");
    }
    if target.contains('@') {
        return Err("target must not contain credentials");
    }
    if target.contains('/') || target.contains('?') || target.contains('#') {
        return Err("target must not contain a path, query, or fragment");
    }

    let (host, port) = if let Some(rest) = target.strip_prefix('[') {
        let end = rest.find(']').ok_or("IPv6 target must be [ADDR]:PORT")?;
        let host = &rest[..end];
        let suffix = &rest[end + 1..];
        let port = suffix
            .strip_prefix(':')
            .ok_or("IPv6 target must be [ADDR]:PORT")?;
        if host.parse::<Ipv6Addr>().is_err() {
            return Err("bracketed target must contain an IPv6 address");
        }
        (host, parse_port(port)?)
    } else {
        if target.matches(':').count() > 1 {
            return Err("IPv6 target must be bracketed as [ADDR]:PORT");
        }
        let idx = target.rfind(':').ok_or("target must be HOST:PORT")?;
        let host = &target[..idx];
        let port = &target[idx + 1..];
        validate_host(host)?;
        (host, parse_port(port)?)
    };

    Ok(HostPort {
        host: host.to_string(),
        port,
    })
}

pub fn validate_host_port_target(target: &str) -> Result<String, String> {
    parse_host_port_target(target)
        .map(|hp| hp.target())
        .map_err(|e| e.to_string())
}

fn parse_port(port: &str) -> Result<u16, &'static str> {
    let port = port.parse::<u16>().map_err(|_| "port must be 1-65535")?;
    if port == 0 {
        return Err("port must be 1-65535");
    }
    Ok(port)
}

fn validate_host(host: &str) -> Result<(), &'static str> {
    if host.is_empty() {
        return Err("host must not be empty");
    }
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    if !host.is_ascii() {
        return Err("host must be ASCII");
    }
    if host.len() > 253 {
        return Err("host is too long");
    }
    for label in host.split('.') {
        if label.is_empty() {
            return Err("host labels must not be empty");
        }
        if label.len() > 63 {
            return Err("host label is too long");
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("host labels must not start or end with '-'");
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("host contains invalid characters");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hostname_ipv4_and_bracketed_ipv6() {
        assert_eq!(
            parse_host_port_target("example.com:443").unwrap(),
            HostPort {
                host: "example.com".to_string(),
                port: 443,
            }
        );
        assert_eq!(
            parse_host_port_target("127.0.0.1:8443").unwrap().target(),
            "127.0.0.1:8443"
        );
        assert_eq!(
            parse_host_port_target("[::1]:443").unwrap().target(),
            "[::1]:443"
        );
    }

    #[test]
    fn rejects_urlish_or_ambiguous_targets() {
        for target in [
            "https://example.com:443",
            "user:pass@example.com:443",
            "example.com:443/path",
            "example.com",
            "example.com:0",
            "::1:443",
        ] {
            assert!(parse_host_port_target(target).is_err(), "{target}");
        }
    }
}

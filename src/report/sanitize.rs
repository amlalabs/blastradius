//! Output sanitization for untrusted local names/paths before rendering.
//!
//! Redaction removes secret-shaped values. This layer removes terminal control
//! and bidi formatting characters that can make local filenames, branch names,
//! env names, or command strings mislead terminal/Markdown/JSON readers.

use serde_json::Value;

fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

fn sanitize_text(s: &str, keep_newlines: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if keep_newlines && c == '\n' {
            out.push('\n');
        } else if c == '\t' {
            out.push(' ');
        } else if c.is_control() || is_bidi_control(c) {
            out.push('?');
        } else {
            out.push(c);
        }
    }
    out
}

/// Sanitize a single-line field for terminal/report rendering.
pub fn inline(s: &str) -> String {
    sanitize_text(s, false)
}

/// Sanitize a whole rendered document while preserving renderer newlines.
pub fn block(s: &str) -> String {
    sanitize_text(s, true)
}

/// Escape dynamic Markdown text after single-line sanitization.
pub fn markdown_text(s: &str) -> String {
    let mut out = String::new();
    for c in inline(s).chars() {
        match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Render a dynamic single-line value as a Markdown code span.
pub fn markdown_code_span(s: &str) -> String {
    format!("`{}`", inline(s).replace('`', "'"))
}

/// Recursively sanitize string values before JSON serialization.
pub fn json_value(v: &mut Value) {
    match v {
        Value::String(s) => {
            *s = inline(s);
        }
        Value::Array(items) => {
            for item in items {
                json_value(item);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                json_value(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inline_removes_control_and_bidi_chars() {
        assert_eq!(inline("ok\x1b[2J\nnext\r\u{202E}"), "ok?[2J?next??");
    }

    #[test]
    fn block_preserves_only_newlines() {
        assert_eq!(block("a\nb\rc\t"), "a\nb?c ");
    }

    #[test]
    fn markdown_escapes_dynamic_text() {
        assert_eq!(
            markdown_text("a|*b* `c` <d>"),
            "a\\|\\*b\\* \\`c\\` \\<d\\>"
        );
    }

    #[test]
    fn json_sanitizes_nested_strings() {
        let mut v = json!({ "a": ["x\x1b", { "b": "y\nz" }] });
        json_value(&mut v);
        assert_eq!(v, json!({ "a": ["x?", { "b": "y?z" }] }));
    }
}

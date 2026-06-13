//! Bounded local file reads for metadata-only probes.

use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CappedReadError {
    NotFound,
    NotFile,
    TooLarge,
    Unreadable,
}

impl CappedReadError {
    pub fn reason(self) -> &'static str {
        match self {
            CappedReadError::NotFound => "not found",
            CappedReadError::NotFile => "not a regular file",
            CappedReadError::TooLarge => "file exceeds size cap; not parsed",
            CappedReadError::Unreadable => "unreadable",
        }
    }
}

/// Read a text file with a hard byte cap. The cap is enforced both before
/// opening and while reading, so a file that grows after `metadata()` cannot
/// force an unbounded allocation.
pub fn read_to_string_capped(path: &Path, max_bytes: u64) -> Result<String, CappedReadError> {
    let meta = match std::fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CappedReadError::NotFound);
        }
        Err(_) => return Err(CappedReadError::Unreadable),
    };
    if !meta.is_file() {
        return Err(CappedReadError::NotFile);
    }
    if meta.len() > max_bytes {
        return Err(CappedReadError::TooLarge);
    }

    let mut file = std::fs::File::open(path).map_err(|_| CappedReadError::Unreadable)?;
    let mut buf = Vec::new();
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|_| CappedReadError::Unreadable)?;
    if buf.len() as u64 > max_bytes {
        return Err(CappedReadError::TooLarge);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capped_read_rejects_oversized_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("large.txt");
        std::fs::write(&path, "abcdef").unwrap();

        assert_eq!(
            read_to_string_capped(&path, 5).unwrap_err(),
            CappedReadError::TooLarge
        );
    }

    #[test]
    fn capped_read_accepts_file_at_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("small.txt");
        std::fs::write(&path, "abcde").unwrap();

        assert_eq!(read_to_string_capped(&path, 5).unwrap(), "abcde");
    }
}

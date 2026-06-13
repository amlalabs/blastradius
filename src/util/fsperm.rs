//! Shared read-only filesystem-permission inference (unix). Decides whether a
//! path is writable/creatable by an identity using DAC mode bits + ownership,
//! WITHOUT ever writing. The identity is the owner of `$HOME` (the running user
//! by definition), avoiding any geteuid/libc dependency. Mirrors the model used
//! by the write_reach / sandbox_integrity probes; does NOT consult ACLs,
//! immutable attrs, or bwrap overlays.

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

/// Resolve the (uid, gid) identity from the home directory's ownership.
pub fn home_identity(home: &Path) -> Option<(u32, u32)> {
    std::fs::metadata(home).ok().map(|m| (m.uid(), m.gid()))
}

fn writable_meta(meta: &std::fs::Metadata, uid: u32, gid: u32) -> bool {
    let mode = meta.permissions().mode();
    (mode & 0o200 != 0 && meta.uid() == uid)
        || (mode & 0o020 != 0 && meta.gid() == gid)
        || (mode & 0o002 != 0)
}

/// Writable by a principal *other* than the owner (world-writable, or
/// group-writable where the file isn't owned by us) — a sharing red flag.
fn nonowner_writable_meta(meta: &std::fs::Metadata, uid: u32) -> bool {
    let mode = meta.permissions().mode();
    (mode & 0o002 != 0) || (mode & 0o020 != 0 && meta.uid() != uid)
}

/// A directory is replaceable-into only if writable AND searchable.
pub fn dir_writable_searchable(path: &Path, uid: u32, gid: u32) -> bool {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    if !writable_meta(&meta, uid, gid) {
        return false;
    }
    let mode = meta.permissions().mode();
    (mode & 0o100 != 0 && meta.uid() == uid)
        || (mode & 0o010 != 0 && meta.gid() == gid)
        || (mode & 0o001 != 0)
}

/// Outcome of a single path writability check (symlinks followed for the
/// writability decision; `is_symlink` records the link itself).
pub struct PathCheck {
    pub exists: bool,
    pub writable: bool,
    pub creatable: bool,
    pub is_symlink: bool,
    pub nonowner_writable: bool,
}

/// Check a path read-only. Absent paths report `creatable` based on the parent.
pub fn check(path: &Path, uid: u32, gid: u32) -> PathCheck {
    match std::fs::symlink_metadata(path) {
        Ok(lmeta) => {
            let is_symlink = lmeta.file_type().is_symlink();
            match std::fs::metadata(path) {
                Ok(t) => PathCheck {
                    exists: true,
                    writable: writable_meta(&t, uid, gid),
                    creatable: false,
                    is_symlink,
                    nonowner_writable: nonowner_writable_meta(&t, uid),
                },
                // Broken symlink -> target absent; not writable.
                Err(_) => PathCheck {
                    exists: true,
                    writable: false,
                    creatable: false,
                    is_symlink,
                    nonowner_writable: false,
                },
            }
        }
        Err(_) => {
            let creatable = path
                .parent()
                .map(|p| dir_writable_searchable(p, uid, gid))
                .unwrap_or(false);
            PathCheck {
                exists: false,
                writable: false,
                creatable,
                is_symlink: false,
                nonowner_writable: false,
            }
        }
    }
}

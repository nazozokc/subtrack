//! Safe path resolution — port of `lib/src/path.ts`.

use std::path::{Component, Path, PathBuf};

/// Default trusted base directories: home and temp.
pub fn default_bases() -> Vec<PathBuf> {
    vec![crate::crypto::home_dir(), std::env::temp_dir()]
}

/// Port of `safePath` (requires the path to exist).
pub fn safe_path(user_path: &str) -> Option<PathBuf> {
    resolve_safe_path(&default_bases(), user_path)
}

/// Port of `safeOutputPath` (path need not exist).
pub fn safe_output_path(target_path: &str) -> Option<PathBuf> {
    resolve_safe_output_path(&default_bases(), target_path)
}

fn is_within(child: &Path, parent: &Path) -> bool {
    if child == parent {
        return true;
    }
    child.starts_with(parent)
}

fn resolve_base(base: &Path) -> PathBuf {
    std::fs::canonicalize(base).unwrap_or_else(|_| normalize(base))
}

/// Lexical normalization (mirror of node path.normalize for absolute paths).
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Absolute, lexically normalized form of `p` relative to the cwd.
fn absolute(p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        normalize(path)
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        normalize(&cwd.join(path))
    }
}

/// Port of `resolveSafePath`.
pub fn resolve_safe_path(bases: &[PathBuf], user_path: &str) -> Option<PathBuf> {
    for base in bases {
        if !base.is_absolute() {
            continue;
        }
        let Ok(resolved) = std::fs::canonicalize(user_path) else {
            continue;
        };
        let base = resolve_base(base);
        if !is_within(&resolved, &base) {
            continue;
        }
        return Some(resolved);
    }
    None
}

/// Port of `resolveSafeOutputPath`.
pub fn resolve_safe_output_path(bases: &[PathBuf], target_path: &str) -> Option<PathBuf> {
    let abs = absolute(target_path);
    for base in bases {
        if !base.is_absolute() {
            continue;
        }
        let base = resolve_base(base);

        // Walk up from the target to the nearest existing parent.
        let mut check = abs.clone();
        while !check.exists() {
            match check.parent() {
                Some(parent) if parent != check => check = parent.to_path_buf(),
                _ => break,
            }
        }

        if !check.exists() {
            if is_within(&abs, &base) || abs == base {
                return Some(abs);
            }
            continue;
        }

        let Ok(resolved) = std::fs::canonicalize(&check) else {
            continue;
        };
        if !is_within(&resolved, &base) {
            continue;
        }
        let remaining = abs
            .strip_prefix(&check)
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let full = if remaining.as_os_str().is_empty() {
            resolved
        } else {
            resolved.join(remaining)
        };
        return Some(full);
    }
    None
}

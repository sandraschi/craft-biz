//! File-system confinement for craft apps (`--allow-path`).
//!
//! `None` = unrestricted (interactive CLI). `Some(empty)` = no file access
//! (MCP server started without `--allow-path`). `Some(roots)` = only paths that
//! canonicalize under one of the roots.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Confinement(pub Option<Vec<PathBuf>>);

impl Confinement {
    pub fn roots(paths: &[PathBuf]) -> Result<Confinement, String> {
        let mut out = Vec::new();
        for p in paths {
            out.push(plain(
                p.canonicalize()
                    .map_err(|e| format!("--allow-path {}: {e}", p.display()))?,
            ));
        }
        Ok(Confinement(Some(out)))
    }

    /// Resolve `path` for reading (must exist; file or directory) or writing
    /// (parent must exist), refusing anything outside the allowed roots.
    pub fn check(&self, path: &str, write: bool) -> Result<PathBuf, String> {
        if path.trim().is_empty() || path.len() > 1024 {
            return Err("path empty or too long".into());
        }
        let p = Path::new(path);
        let resolved = if write {
            let name = p.file_name().ok_or("path has no file name")?;
            let parent = match p.parent() {
                Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
                _ => PathBuf::from("."),
            };
            parent
                .canonicalize()
                .map_err(|e| format!("{}: {e}", parent.display()))?
                .join(name)
        } else {
            p.canonicalize().map_err(|e| format!("{path}: {e}"))?
        };
        let resolved = plain(resolved);
        match &self.0 {
            None => Ok(resolved),
            Some(roots) if roots.iter().any(|r| resolved.starts_with(r)) => Ok(resolved),
            Some(roots) if roots.is_empty() => {
                Err("file access disabled: start the MCP server with --allow-path <DIR>".into())
            }
            Some(_) => Err(format!(
                "{} is outside the allowed paths",
                resolved.display()
            )),
        }
    }
}

/// Drop the Windows verbatim prefix that `canonicalize` adds (`\\?\D:\x` -> `D:\x`)
/// so paths shown to users and agents look normal. UNC verbatim paths are kept.
pub fn plain(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confinement_modes() {
        let root = std::env::temp_dir().join(format!("craft-biz-confine-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let inside = root.join("a.txt");
        std::fs::write(&inside, "x").unwrap();
        let path = inside.display().to_string();
        assert!(Confinement(None).check(&path, false).is_ok());
        assert!(Confinement(Some(Vec::new()))
            .check(&path, false)
            .unwrap_err()
            .contains("--allow-path"));
        let c = Confinement::roots(std::slice::from_ref(&root)).unwrap();
        assert!(c.check(&path, false).is_ok());
        assert!(
            c.check(&root.display().to_string(), false).is_ok(),
            "directories are readable"
        );
        assert!(c
            .check(&root.join("new.csv").display().to_string(), true)
            .is_ok());
        let outside = std::env::temp_dir().display().to_string();
        assert!(c.check(&outside, false).unwrap_err().contains("outside"));
        assert!(c.check("", false).is_err());
        assert!(!c
            .check(&path, false)
            .unwrap()
            .display()
            .to_string()
            .starts_with(r"\\?\"));
        assert_eq!(plain(PathBuf::from(r"\\?\D:\x")), PathBuf::from(r"D:\x"));
        assert_eq!(
            plain(PathBuf::from(r"\\?\UNC\srv\s")),
            PathBuf::from(r"\\?\UNC\srv\s")
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}

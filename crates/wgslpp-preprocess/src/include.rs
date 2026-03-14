use std::path::{Path, PathBuf};

use crate::packages::PackageRegistry;

/// The kind of include directive.
#[derive(Debug, Clone, PartialEq)]
pub enum IncludeKind {
    /// `#include "relative/path.wgsl"` — resolved relative to the including file
    Relative(String),
    /// `#include <package/path.wgsl>` — resolved via package registry
    Package(String),
}

/// Parse an `#include` directive, returning the include kind.
/// `rest` is everything after `#include `.
pub fn parse_include(rest: &str) -> Result<IncludeKind, String> {
    let rest = rest.trim();
    if rest.starts_with('"') {
        let end = rest[1..]
            .find('"')
            .ok_or_else(|| "unclosed \" in #include".to_string())?;
        let path = &rest[1..1 + end];
        if path.is_empty() {
            return Err("empty path in #include".into());
        }
        Ok(IncludeKind::Relative(path.to_string()))
    } else if rest.starts_with('<') {
        let end = rest[1..]
            .find('>')
            .ok_or_else(|| "unclosed < in #include".to_string())?;
        let path = &rest[1..1 + end];
        if path.is_empty() {
            return Err("empty path in #include".into());
        }
        Ok(IncludeKind::Package(path.to_string()))
    } else {
        Err(format!("invalid #include syntax: {}", rest))
    }
}

/// Resolve an include path to an absolute file path.
pub fn resolve_include(
    kind: &IncludeKind,
    current_file: &Path,
    packages: &PackageRegistry,
) -> Result<PathBuf, String> {
    match kind {
        IncludeKind::Relative(path) => {
            let base = current_file
                .parent()
                .ok_or_else(|| "cannot determine parent directory".to_string())?;
            let resolved = base.join(path);
            if resolved.exists() {
                Ok(resolved)
            } else {
                Err(format!(
                    "included file not found: {} (resolved to {})",
                    path,
                    resolved.display()
                ))
            }
        }
        IncludeKind::Package(path) => packages
            .resolve(path)
            .and_then(|p| if p.exists() { Some(p) } else { None })
            .ok_or_else(|| format!("cannot resolve package include: {}", path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative() {
        let kind = parse_include("\"common/math.wgsl\"").unwrap();
        assert_eq!(kind, IncludeKind::Relative("common/math.wgsl".to_string()));
    }

    #[test]
    fn test_parse_package() {
        let kind = parse_include("<polymer/common/math.wgsl>").unwrap();
        assert_eq!(
            kind,
            IncludeKind::Package("polymer/common/math.wgsl".to_string())
        );
    }

    #[test]
    fn test_parse_errors() {
        assert!(parse_include("\"unclosed").is_err());
        assert!(parse_include("<unclosed").is_err());
        assert!(parse_include("bare_path").is_err());
        assert!(parse_include("\"\"").is_err());
    }
}

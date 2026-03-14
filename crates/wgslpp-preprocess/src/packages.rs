use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Named package registry for `#include <package/path>` resolution.
#[derive(Debug, Clone, Default)]
pub struct PackageRegistry {
    /// Map from package name to its root directory.
    packages: HashMap<String, PathBuf>,
}

impl PackageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a package: `name` maps to `root_path`.
    pub fn add(&mut self, name: impl Into<String>, root_path: impl Into<PathBuf>) {
        self.packages.insert(name.into(), root_path.into());
    }

    /// Resolve `<package/rest/of/path.wgsl>`.
    /// The first path component is the package name, the rest is the path within it.
    pub fn resolve(&self, include_path: &str) -> Option<PathBuf> {
        let include_path = include_path.trim();
        // Split on first '/'
        let (pkg_name, rest) = include_path.split_once('/')?;
        let root = self.packages.get(pkg_name)?;
        let resolved = root.join(rest);
        Some(resolved)
    }

    /// Resolve a relative include path against a base directory.
    pub fn resolve_relative(base_dir: &Path, include_path: &str) -> PathBuf {
        base_dir.join(include_path.trim())
    }
}

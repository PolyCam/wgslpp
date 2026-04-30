use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::packages::PackageRegistry;

/// Configuration loaded from wgslpp.json.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct WgslppConfig {
    #[serde(default)]
    pub packages: Vec<PackageConfig>,
    #[serde(default, rename = "manifestDir")]
    pub manifest_dir: Option<String>,
    #[serde(default)]
    pub configurations: HashMap<String, ConfigurationDef>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PackageConfig {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ConfigurationDef {
    #[serde(default)]
    pub defines: HashMap<String, String>,
}

impl WgslppConfig {
    /// Load a config file from disk.
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Build a PackageRegistry from the config, resolving relative paths
    /// against `base_dir` (typically the directory containing wgslpp.json).
    pub fn to_packages(&self, base_dir: &Path) -> PackageRegistry {
        let mut registry = PackageRegistry::new();
        for pkg in &self.packages {
            let pkg_path = if Path::new(&pkg.path).is_absolute() {
                PathBuf::from(&pkg.path)
            } else {
                base_dir.join(&pkg.path)
            };
            registry.add(pkg.name.clone(), pkg_path);
        }
        registry
    }
}

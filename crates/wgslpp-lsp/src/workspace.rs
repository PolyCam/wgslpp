//! Workspace model: include graph tracking, config loading, preprocessed snapshots.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wgslpp_preprocess::packages::PackageRegistry;
use wgslpp_preprocess::source_map::SourceMap;

/// Configuration loaded from wgslpp.json.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct WorkspaceConfig {
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

/// A preprocessed snapshot of a file.
pub struct PreprocessedSnapshot {
    pub code: String,
    pub source_map: SourceMap,
    /// Parsed + validated naga module (if validation succeeded).
    pub module: Option<naga::Module>,
    pub module_info: Option<naga::valid::ModuleInfo>,
}

/// Workspace state tracking open documents and their preprocessed forms.
pub struct Workspace {
    /// Workspace root directory.
    pub root: PathBuf,
    /// Package registry from config.
    pub packages: PackageRegistry,
    /// Active defines for the current configuration.
    pub defines: HashMap<String, String>,
    /// Open documents: URI string -> source text.
    pub documents: HashMap<String, String>,
    /// Cached preprocessed snapshots: URI string -> snapshot.
    pub snapshots: HashMap<String, PreprocessedSnapshot>,
    /// Available configurations from config file.
    pub configurations: HashMap<String, ConfigurationDef>,
    /// Currently active configuration name.
    pub active_config: Option<String>,
}

impl Workspace {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            packages: PackageRegistry::new(),
            defines: HashMap::new(),
            documents: HashMap::new(),
            snapshots: HashMap::new(),
            configurations: HashMap::new(),
            active_config: None,
        }
    }

    /// Load configuration from wgslpp.json if it exists.
    pub fn load_config(&mut self) {
        let config_path = self.root.join("wgslpp.json");
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<WorkspaceConfig>(&content) {
                for pkg in &config.packages {
                    let pkg_path = if Path::new(&pkg.path).is_absolute() {
                        PathBuf::from(&pkg.path)
                    } else {
                        self.root.join(&pkg.path)
                    };
                    self.packages.add(pkg.name.clone(), pkg_path);
                }
                self.configurations = config.configurations;

                // Activate first configuration if available
                if let Some(first_name) = self.configurations.keys().next().cloned() {
                    self.activate_config(&first_name);
                }

                log::info!("Loaded config from {}", config_path.display());
            }
        }
    }

    /// Activate a named configuration.
    pub fn activate_config(&mut self, name: &str) {
        if let Some(config) = self.configurations.get(name) {
            self.defines = config.defines.clone();
            self.active_config = Some(name.to_string());
            self.snapshots.clear();
        }
    }

    /// Update a document's source text and reprocess it.
    pub fn update_document(&mut self, uri: &str, text: String) {
        self.documents.insert(uri.to_string(), text);
        self.reprocess(uri);
    }

    /// Remove a document.
    pub fn close_document(&mut self, uri: &str) {
        self.documents.remove(uri);
        self.snapshots.remove(uri);
    }

    /// Reprocess a single document.
    pub fn reprocess(&mut self, uri: &str) {
        let Some(source) = self.documents.get(uri) else {
            return;
        };

        let file_path = uri_to_path(uri);

        let config = wgslpp_preprocess::PreprocessConfig {
            packages: self.packages.clone(),
            defines: self.defines.clone(),
        };

        let pp_result = if let Some(path) = &file_path {
            if path.exists() {
                wgslpp_preprocess::preprocess(path, &config)
            } else {
                wgslpp_preprocess::preprocess_str(
                    source,
                    path.to_str().unwrap_or("untitled.wgsl"),
                    &config,
                )
            }
        } else {
            wgslpp_preprocess::preprocess_str(source, "untitled.wgsl", &config)
        };

        match pp_result {
            Ok(pp) => {
                let validation = wgslpp_core::validate::validate(&pp.code, Some(&pp.source_map));
                self.snapshots.insert(
                    uri.to_string(),
                    PreprocessedSnapshot {
                        code: pp.code,
                        source_map: pp.source_map,
                        module: validation.module,
                        module_info: validation.module_info,
                    },
                );
            }
            Err(_) => {
                self.snapshots.insert(
                    uri.to_string(),
                    PreprocessedSnapshot {
                        code: source.clone(),
                        source_map: SourceMap::new(),
                        module: None,
                        module_info: None,
                    },
                );
            }
        }
    }

    /// Get the preprocessed snapshot for a document.
    pub fn get_snapshot(&self, uri: &str) -> Option<&PreprocessedSnapshot> {
        self.snapshots.get(uri)
    }
}

/// Convert a file URI to a filesystem path.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    if let Some(path) = uri.strip_prefix("file://") {
        Some(PathBuf::from(path))
    } else {
        None
    }
}

/// Convert a filesystem path to a file URI.
pub fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

//! Workspace model: include graph tracking, config loading, preprocessed snapshots.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wgslpp_preprocess::config::{ConfigurationDef, WgslppConfig};
use wgslpp_preprocess::packages::PackageRegistry;
use wgslpp_preprocess::source_map::SourceMap;

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
        if let Ok(config) = WgslppConfig::load(&config_path) {
            self.packages = config.to_packages(&self.root);
            self.configurations = config.configurations;

            // Activate first configuration if available
            if let Some(first_name) = self.configurations.keys().next().cloned() {
                self.activate_config(&first_name);
            }

            log::info!("Loaded config from {}", config_path.display());
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
///
/// Handles both Unix-style URIs (`file:///foo/bar`) and Windows-style
/// (`file:///C:/foo/bar`). On Windows, the leading slash before the drive
/// letter is part of the URI's path component and is dropped so the result
/// is a usable filesystem path.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let stripped = uri.strip_prefix("file://")?;
    // `file:///C:/foo` -> stripped is `/C:/foo`; we want `C:/foo`. The drive
    // letter signature is `/X:` at the start.
    let bytes = stripped.as_bytes();
    let path = if bytes.first() == Some(&b'/')
        && bytes.get(2) == Some(&b':')
        && bytes.get(1).is_some_and(|b| b.is_ascii_alphabetic())
    {
        &stripped[1..]
    } else {
        stripped
    };
    Some(PathBuf::from(path))
}

/// Convert a filesystem path to a file URI.
///
/// On Unix this is `file://` + the path. On Windows the path uses
/// backslashes which aren't valid URI characters, so we normalise to
/// forward slashes and prepend an extra `/` so the drive letter ends up
/// after the authority (e.g. `C:\foo` → `file:///C:/foo`). We also strip
/// the `\\?\` extended-length prefix that `Path::canonicalize` returns on
/// Windows — it'd otherwise yield `file:////?/C:/foo`, which most URI
/// parsers reject.
pub fn path_to_uri(path: &Path) -> String {
    let raw = path.display().to_string();
    let normalized: String = raw.replace('\\', "/");
    let trimmed = normalized
        .strip_prefix("//?/")
        .or_else(|| normalized.strip_prefix("//./"))
        .unwrap_or(&normalized);
    if trimmed.starts_with('/') {
        format!("file://{}", trimmed)
    } else {
        // Windows drive-letter form: `C:/foo` -> `file:///C:/foo`.
        format!("file:///{}", trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_path_roundtrip() {
        let p = PathBuf::from("/tmp/foo/bar.wgsl");
        let uri = path_to_uri(&p);
        assert_eq!(uri, "file:///tmp/foo/bar.wgsl");
        assert_eq!(uri_to_path(&uri).unwrap(), p);
    }

    #[test]
    fn windows_drive_path_roundtrip() {
        // Spelled with forward slashes since `Path` accepts both on all
        // platforms — we only need to verify the URI-shape logic, which
        // doesn't care about the OS.
        let raw = "C:/Users/runner/file.wgsl";
        let uri = path_to_uri(Path::new(raw));
        assert_eq!(uri, "file:///C:/Users/runner/file.wgsl");
        let back = uri_to_path(&uri).unwrap();
        assert_eq!(back.to_string_lossy().replace('\\', "/"), raw);
    }

    #[test]
    fn windows_extended_path_is_stripped() {
        // `\\?\C:\foo` is what canonicalize returns on Windows.
        let p = "\\\\?\\C:\\Users\\runner\\file.wgsl";
        let uri = path_to_uri(Path::new(p));
        assert_eq!(uri, "file:///C:/Users/runner/file.wgsl");
    }
}

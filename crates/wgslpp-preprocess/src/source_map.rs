use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Maps output lines back to original source file locations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceMap {
    /// Interned file paths.
    pub files: Vec<PathBuf>,
    /// One entry per output line (0-based index = output line number).
    pub entries: Vec<SourceLocation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Index into `files`.
    pub file_idx: u32,
    /// 1-based line number in the source file.
    pub line: u32,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a file path, returning its index.
    pub fn intern_file(&mut self, path: &Path) -> u32 {
        if let Some(idx) = self.files.iter().position(|p| p == path) {
            return idx as u32;
        }
        let idx = self.files.len() as u32;
        self.files.push(path.to_path_buf());
        idx
    }

    /// Record that the next output line came from `file_idx` at `line` (1-based).
    pub fn push(&mut self, file_idx: u32, line: u32) {
        self.entries.push(SourceLocation { file_idx, line });
    }

    /// Look up where an output line (0-based) came from.
    pub fn lookup(&self, output_line: usize) -> Option<(&Path, u32)> {
        let loc = self.entries.get(output_line)?;
        let path = self.files.get(loc.file_idx as usize)?;
        Some((path, loc.line))
    }

    /// Serialize to the JSON format: { "files": [...], "entries": [[out, file, src], ...] }
    pub fn to_json(&self) -> serde_json::Value {
        let files: Vec<&str> = self.files.iter().map(|p| p.to_str().unwrap_or("")).collect();
        let entries: Vec<[u32; 3]> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, loc)| [i as u32, loc.file_idx, loc.line])
            .collect();
        serde_json::json!({ "files": files, "entries": entries })
    }
}

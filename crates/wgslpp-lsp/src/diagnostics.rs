//! Convert preprocessor and naga validation errors to LSP diagnostics.

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::workspace::Workspace;

/// Generate LSP diagnostics for a document.
pub fn compute_diagnostics(workspace: &Workspace, uri: &str) -> Vec<Diagnostic> {
    let source = match workspace.documents.get(uri) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let file_path = crate::workspace::uri_to_path(uri);
    let config = wgslpp_preprocess::PreprocessConfig {
        packages: workspace.packages.clone(),
        defines: workspace.defines.clone(),
    };

    // 1. Try preprocessing
    let pp_result = if let Some(ref path) = file_path {
        if path.exists() {
            wgslpp_preprocess::preprocess(path, &config)
        } else {
            wgslpp_preprocess::preprocess_str(source, path.to_str().unwrap_or(""), &config)
        }
    } else {
        wgslpp_preprocess::preprocess_str(source, "untitled.wgsl", &config)
    };

    let pp = match pp_result {
        Ok(pp) => pp,
        Err(e) => {
            // Preprocessor error — report it
            return vec![preprocess_error_to_diagnostic(&e, source)];
        }
    };

    // 2. Validate with naga
    let validation = wgslpp_core::validate::validate(&pp.code, Some(&pp.source_map));

    validation
        .diagnostics
        .iter()
        .map(|d| {
            let severity = match d.severity {
                wgslpp_core::validate::Severity::Error => DiagnosticSeverity::ERROR,
                wgslpp_core::validate::Severity::Warning => DiagnosticSeverity::WARNING,
            };

            // Use remapped location if available, otherwise try to extract from message
            let range = if let (Some(ref _file), Some(line)) = (&d.file, d.line) {
                let line0 = line.saturating_sub(1);
                let col = d.column.unwrap_or(1).saturating_sub(1);
                Range::new(Position::new(line0, col), Position::new(line0, col + 1))
            } else if let Some(line) = d.line {
                let line0 = line.saturating_sub(1);
                Range::new(Position::new(line0, 0), Position::new(line0, 1000))
            } else {
                Range::new(Position::new(0, 0), Position::new(0, 0))
            };

            Diagnostic {
                range,
                severity: Some(severity),
                source: Some("wgslpp".to_string()),
                message: d.message.clone(),
                ..Default::default()
            }
        })
        .collect()
}

fn preprocess_error_to_diagnostic(
    error: &wgslpp_preprocess::PreprocessError,
    _source: &str,
) -> Diagnostic {
    let (line, message) = match error {
        wgslpp_preprocess::PreprocessError::Directive { line, message, .. } => {
            (*line, message.clone())
        }
        wgslpp_preprocess::PreprocessError::Io { path, source } => {
            (0, format!("I/O error reading {}: {}", path.display(), source))
        }
        wgslpp_preprocess::PreprocessError::CircularInclude { path } => {
            (0, format!("circular include: {}", path.display()))
        }
    };

    let line0 = line.saturating_sub(1);
    Diagnostic {
        range: Range::new(Position::new(line0, 0), Position::new(line0, 1000)),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("wgslpp".to_string()),
        message,
        ..Default::default()
    }
}

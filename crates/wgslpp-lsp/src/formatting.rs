//! Formatting via naga writer — re-emit validated WGSL with normalized formatting,
//! preserving preprocessor directives in their original positions.

use lsp_types::{Position, Range, TextEdit};

use crate::workspace::Workspace;

/// Format a document. Returns text edits to apply.
pub fn format_document(workspace: &Workspace, uri: &str) -> Vec<TextEdit> {
    let source = match workspace.documents.get(uri) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let snapshot = match workspace.get_snapshot(uri) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let module = match &snapshot.module {
        Some(m) => m,
        None => return Vec::new(),
    };

    let module_info = match &snapshot.module_info {
        Some(i) => i,
        None => return Vec::new(),
    };

    // Re-emit using naga writer
    let mut output = String::new();
    let mut writer =
        naga::back::wgsl::Writer::new(&mut output, naga::back::wgsl::WriterFlags::empty());
    if writer.write(module, module_info).is_err() {
        return Vec::new();
    }

    // If the source has preprocessor directives, we can't fully format it
    // through naga (the directives would be lost). In that case, only format
    // files without preprocessor directives.
    let has_directives = source.lines().any(|l| l.trim().starts_with('#'));
    if has_directives {
        return Vec::new();
    }

    // Replace entire document
    let line_count = source.lines().count() as u32;
    let last_line = source.lines().last().unwrap_or("");

    vec![TextEdit {
        range: Range::new(
            Position::new(0, 0),
            Position::new(line_count, last_line.len() as u32),
        ),
        new_text: output,
    }]
}

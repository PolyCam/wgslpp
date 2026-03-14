//! Document symbols and workspace symbols from naga IR.

use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

use crate::workspace::Workspace;

/// Extract document symbols from a document's naga module.
#[allow(deprecated)] // DocumentSymbol::deprecated field
pub fn document_symbols(workspace: &Workspace, uri: &str) -> Vec<DocumentSymbol> {
    let source = match workspace.documents.get(uri) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let snapshot = match workspace.get_snapshot(uri) {
        Some(s) => s,
        None => return symbols_from_text(source),
    };

    let module = match &snapshot.module {
        Some(m) => m,
        None => return symbols_from_text(source),
    };

    let mut symbols = Vec::new();

    // Entry points
    for ep in &module.entry_points {
        let stage_str = match ep.stage {
            naga::ShaderStage::Vertex => "@vertex",
            naga::ShaderStage::Fragment => "@fragment",
            naga::ShaderStage::Compute => "@compute",
            _ => "",
        };
        if let Some(range) = find_name_in_source(source, &ep.name) {
            symbols.push(DocumentSymbol {
                name: ep.name.clone(),
                detail: Some(format!("{} fn", stage_str)),
                kind: SymbolKind::FUNCTION,
                range,
                selection_range: range,
                children: None,
                tags: None,
                deprecated: None,
            });
        }
    }

    // Functions
    for (_, func) in module.functions.iter() {
        if let Some(ref name) = func.name {
            if let Some(range) = find_name_in_source(source, name) {
                symbols.push(DocumentSymbol {
                    name: name.clone(),
                    detail: Some("fn".to_string()),
                    kind: SymbolKind::FUNCTION,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None,
                });
            }
        }
    }

    // Structs
    for (_, ty) in module.types.iter() {
        if let Some(ref name) = ty.name {
            if let naga::TypeInner::Struct { ref members, .. } = ty.inner {
                let member_symbols: Vec<DocumentSymbol> = members
                    .iter()
                    .filter_map(|m| {
                        let member_name = m.name.as_ref()?;
                        let range = find_name_in_source(source, member_name)?;
                        Some(DocumentSymbol {
                            name: member_name.clone(),
                            detail: None,
                            kind: SymbolKind::FIELD,
                            range,
                            selection_range: range,
                            children: None,
                            tags: None,
                            deprecated: None,
                        })
                    })
                    .collect();

                if let Some(range) = find_name_in_source(source, name) {
                    symbols.push(DocumentSymbol {
                        name: name.clone(),
                        detail: Some("struct".to_string()),
                        kind: SymbolKind::STRUCT,
                        range,
                        selection_range: range,
                        children: if member_symbols.is_empty() {
                            None
                        } else {
                            Some(member_symbols)
                        },
                        tags: None,
                        deprecated: None,
                    });
                }
            }
        }
    }

    // Global variables
    for (_, global) in module.global_variables.iter() {
        if let Some(ref name) = global.name {
            let detail = match global.space {
                naga::AddressSpace::Uniform => "uniform",
                naga::AddressSpace::Storage { .. } => "storage",
                naga::AddressSpace::Handle => "handle",
                _ => "var",
            };
            if let Some(range) = find_name_in_source(source, name) {
                symbols.push(DocumentSymbol {
                    name: name.clone(),
                    detail: Some(detail.to_string()),
                    kind: SymbolKind::VARIABLE,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None,
                });
            }
        }
    }

    // Constants
    for (_, constant) in module.constants.iter() {
        if let Some(ref name) = constant.name {
            if let Some(range) = find_name_in_source(source, name) {
                symbols.push(DocumentSymbol {
                    name: name.clone(),
                    detail: Some("const".to_string()),
                    kind: SymbolKind::CONSTANT,
                    range,
                    selection_range: range,
                    children: None,
                    tags: None,
                    deprecated: None,
                });
            }
        }
    }

    symbols
}

/// Find the first occurrence of a name in source, returning its range.
fn find_name_in_source(source: &str, name: &str) -> Option<Range> {
    for (line_num, line) in source.lines().enumerate() {
        if let Some(col) = find_word_in_line(line, name) {
            return Some(Range::new(
                Position::new(line_num as u32, col as u32),
                Position::new(line_num as u32, (col + name.len()) as u32),
            ));
        }
    }
    None
}

/// Find a whole-word match of `name` in `line`.
fn find_word_in_line(line: &str, name: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut pos = 0;
    while let Some(idx) = line[pos..].find(name) {
        let abs_pos = pos + idx;
        let before_ok = abs_pos == 0
            || !(bytes[abs_pos - 1].is_ascii_alphanumeric() || bytes[abs_pos - 1] == b'_');
        let after_pos = abs_pos + name.len();
        let after_ok = after_pos >= bytes.len()
            || !(bytes[after_pos].is_ascii_alphanumeric() || bytes[after_pos] == b'_');
        if before_ok && after_ok {
            return Some(abs_pos);
        }
        pos = abs_pos + 1;
    }
    None
}

/// Fallback: extract symbols from source text using simple pattern matching.
#[allow(deprecated)]
fn symbols_from_text(source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("fn ") {
            if let Some(name) = rest.split('(').next() {
                let name = name.trim();
                if !name.is_empty() {
                    let col = line.find(name).unwrap_or(0);
                    symbols.push(DocumentSymbol {
                        name: name.to_string(),
                        detail: Some("fn".to_string()),
                        kind: SymbolKind::FUNCTION,
                        range: Range::new(
                            Position::new(line_num as u32, col as u32),
                            Position::new(line_num as u32, (col + name.len()) as u32),
                        ),
                        selection_range: Range::new(
                            Position::new(line_num as u32, col as u32),
                            Position::new(line_num as u32, (col + name.len()) as u32),
                        ),
                        children: None,
                        tags: None,
                        deprecated: None,
                    });
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("struct ") {
            let name = rest
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if !name.is_empty() {
                let col = line.find(name).unwrap_or(0);
                symbols.push(DocumentSymbol {
                    name: name.to_string(),
                    detail: Some("struct".to_string()),
                    kind: SymbolKind::STRUCT,
                    range: Range::new(
                        Position::new(line_num as u32, col as u32),
                        Position::new(line_num as u32, (col + name.len()) as u32),
                    ),
                    selection_range: Range::new(
                        Position::new(line_num as u32, col as u32),
                        Position::new(line_num as u32, (col + name.len()) as u32),
                    ),
                    children: None,
                    tags: None,
                    deprecated: None,
                });
            }
        }
    }

    symbols
}

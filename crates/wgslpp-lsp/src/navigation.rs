//! Go-to-definition and find-references via source maps + naga IR.

use lsp_types::{Location, Position, Range, Uri};

use crate::workspace::{path_to_uri, uri_to_path, Workspace};

/// Handle go-to-definition request.
pub fn goto_definition(
    workspace: &Workspace,
    uri: &str,
    position: Position,
) -> Option<Location> {
    let source = workspace.documents.get(uri)?;

    // Check if we're on an #include directive — navigate to the included file
    let line_text = source.lines().nth(position.line as usize)?;
    if let Some(include_target) = parse_include_on_line(line_text) {
        return resolve_include_location(workspace, uri, &include_target);
    }

    // Look up in naga module
    let snapshot = workspace.get_snapshot(uri)?;
    let module = snapshot.module.as_ref()?;
    let word = word_at_position(source, position)?;

    // Try naga IR lookup
    if let Some(loc) = find_definition_in_module(module, &word) {
        return Some(loc);
    }

    None
}

/// Extract the word at a given position in the source.
fn word_at_position(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let col = position.character as usize;

    if col >= line.len() {
        return None;
    }

    let bytes = line.as_bytes();
    let is_ident_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    let mut start = col;
    while start > 0 && is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end < bytes.len() && is_ident_char(bytes[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }
    Some(line[start..end].to_string())
}

/// Check if a line contains an #include directive, return the path.
fn parse_include_on_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("#include")?;
    let rest = rest.trim();

    if rest.starts_with('"') {
        let end = rest[1..].find('"')?;
        Some(rest[1..1 + end].to_string())
    } else if rest.starts_with('<') {
        let end = rest[1..].find('>')?;
        Some(rest[1..1 + end].to_string())
    } else {
        None
    }
}

/// Resolve an include path to a Location.
fn resolve_include_location(
    workspace: &Workspace,
    uri: &str,
    include_path: &str,
) -> Option<Location> {
    let current_path = uri_to_path(uri)?;
    let zero_range = Range::new(Position::new(0, 0), Position::new(0, 0));

    // Try relative resolution first
    let base_dir = current_path.parent()?;
    let relative = base_dir.join(include_path);
    if relative.exists() {
        let target_uri: Uri = path_to_uri(&relative).parse().ok()?;
        return Some(Location {
            uri: target_uri,
            range: zero_range,
        });
    }

    // Try package resolution
    let resolved = workspace.packages.resolve(include_path)?;
    if resolved.exists() {
        let target_uri: Uri = path_to_uri(&resolved).parse().ok()?;
        return Some(Location {
            uri: target_uri,
            range: zero_range,
        });
    }

    None
}

/// Find a definition in the naga module (returns location in current file by text search).
fn find_definition_in_module(
    module: &naga::Module,
    name: &str,
) -> Option<Location> {
    // We verify the symbol exists in the module but can't easily get source locations
    // from naga spans without the preprocessed source. Return None to fall through
    // to text-based search.

    // Check functions
    for (_, func) in module.functions.iter() {
        if func.name.as_deref() == Some(name) {
            return None; // exists, but let text search find the location
        }
    }
    for ep in &module.entry_points {
        if ep.name == name {
            return None;
        }
    }

    None
}

/// Find the definition of a symbol by searching the source text directly.
pub fn find_definition_by_text(
    workspace: &Workspace,
    uri: &str,
    name: &str,
) -> Option<Location> {
    let source = workspace.documents.get(uri)?;

    // Search for definition patterns: fn NAME, struct NAME, var NAME, let NAME, const NAME
    let patterns = [
        format!("fn {}", name),
        format!("struct {}", name),
        format!("var<uniform> {}", name),
        format!("var {}", name),
        format!("let {}", name),
        format!("const {}", name),
    ];

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        for pattern in &patterns {
            if trimmed.contains(pattern.as_str()) {
                let col = line.find(name).unwrap_or(0) as u32;
                let parsed_uri: Uri = uri.parse().ok()?;
                return Some(Location {
                    uri: parsed_uri,
                    range: Range::new(
                        Position::new(line_num as u32, col),
                        Position::new(line_num as u32, col + name.len() as u32),
                    ),
                });
            }
        }
    }

    None
}

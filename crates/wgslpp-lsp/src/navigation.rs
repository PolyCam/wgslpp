//! Go-to-definition and find-references via source maps + naga IR.

use std::path::{Path, PathBuf};

use lsp_types::{Location, Position, Range, Uri};

use crate::workspace::{path_to_uri, uri_to_path, PreprocessedSnapshot, Workspace};

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

    let word = word_at_position(source, position)?;

    // Walk current document + every file pulled in via #include, looking for
    // the definition. The source map is the authoritative include graph for
    // this snapshot, so it covers transitive includes too.
    find_definition_by_text(workspace, uri, &word)
}

/// Extract the word at a given position in the source.
fn word_at_position(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let col = position.character as usize;

    if col > line.len() {
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

/// Find the definition of a symbol by searching the source text directly.
///
/// Looks at the current file first, then every other file the snapshot pulled
/// in via `#include` (tracked through the preprocessor's source map). For each
/// file the search recognises top-level declarations (`fn`, `struct`,
/// `var`/`var<...>`, `let`, `const`) plus parameters and struct fields written
/// as `name:`.
pub fn find_definition_by_text(
    workspace: &Workspace,
    uri: &str,
    name: &str,
) -> Option<Location> {
    // Search the current document first so a local definition wins over a
    // header definition with the same name.
    if let Some(source) = workspace.documents.get(uri) {
        if let Some(loc) = search_text_for_definition(source, uri, name) {
            return Some(loc);
        }
    }

    // Then walk every other file involved in this snapshot's preprocessing.
    let snapshot = workspace.get_snapshot(uri);
    if let Some(snapshot) = snapshot {
        for path in snapshot_other_files(snapshot, uri) {
            if let Some(loc) = search_in_file(workspace, &path, name) {
                return Some(loc);
            }
        }
    }

    None
}

/// Files in the snapshot's include graph excluding the document itself.
fn snapshot_other_files(snapshot: &PreprocessedSnapshot, primary_uri: &str) -> Vec<PathBuf> {
    let primary_path = uri_to_path(primary_uri);
    snapshot
        .source_map
        .files
        .iter()
        .filter(|p| match &primary_path {
            Some(pp) => *p != pp,
            None => true,
        })
        .cloned()
        .collect()
}

/// Read a file (preferring the open document buffer over disk) and search for
/// the definition.
fn search_in_file(workspace: &Workspace, path: &Path, name: &str) -> Option<Location> {
    let uri = path_to_uri(path);
    if let Some(text) = workspace.documents.get(&uri) {
        return search_text_for_definition(text, &uri, name);
    }
    let text = std::fs::read_to_string(path).ok()?;
    search_text_for_definition(&text, &uri, name)
}

/// Search a file's text for a definition of `name`.
fn search_text_for_definition(source: &str, uri: &str, name: &str) -> Option<Location> {
    for (line_num, line) in source.lines().enumerate() {
        if let Some(col) = find_definition_in_line(line, name) {
            let parsed_uri: Uri = uri.parse().ok()?;
            return Some(Location {
                uri: parsed_uri,
                range: Range::new(
                    Position::new(line_num as u32, col as u32),
                    Position::new(line_num as u32, col as u32 + name.len() as u32),
                ),
            });
        }
    }
    None
}

/// Find the column of a definition of `name` on this line, or `None`.
///
/// Recognises:
///   - `fn NAME`, `struct NAME`, `var NAME`, `let NAME`, `const NAME`
///   - `var<...> NAME`
///   - `NAME:` written as a function parameter (`(NAME:` / `, NAME:`) or as
///     a struct field (line indent followed by `NAME:`).
pub(crate) fn find_definition_in_line(line: &str, name: &str) -> Option<usize> {
    if let Some(col) = find_keyword_decl(line, name) {
        return Some(col);
    }
    find_param_or_field(line, name)
}

/// Match `<keyword>[<...>] NAME` declarations anywhere on the line.
///
/// We scan the line because WGSL attributes (`@vertex`, `@fragment`,
/// `@compute`, `@group(...)`, `@binding(...)`) often precede the keyword on
/// the same line, e.g. `@fragment fn fs_main()`.
fn find_keyword_decl(line: &str, name: &str) -> Option<usize> {
    const KEYWORDS: &[&str] = &["fn", "struct", "var", "let", "const", "alias", "override"];

    let bytes = line.as_bytes();
    for kw in KEYWORDS {
        let mut search_from = 0;
        while let Some(rel) = line[search_from..].find(kw) {
            let i = search_from + rel;
            search_from = i + 1;

            // Word boundary on the left.
            let left_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            // Word boundary on the right (must be terminated by non-ident).
            let after_kw = i + kw.len();
            let right_ok = bytes
                .get(after_kw)
                .map(|b| !is_ident_byte(*b))
                .unwrap_or(false);
            if !(left_ok && right_ok) {
                continue;
            }

            let mut j = after_kw;
            // Skip optional `<...>` template (e.g. `var<uniform>`).
            // Leading whitespace allowed: `var <uniform>` is unusual but we'll
            // tolerate it.
            let rest = &line[j..];
            let ws = rest.len() - rest.trim_start().len();
            j += ws;
            if line[j..].starts_with('<') {
                if let Some(end) = line[j..].find('>') {
                    j += end + 1;
                } else {
                    continue;
                }
            }
            // Skip whitespace before identifier.
            let rest = &line[j..];
            let ws = rest.len() - rest.trim_start().len();
            let name_start = j + ws;
            if line[name_start..].starts_with(name) {
                let after = name_start + name.len();
                let after_ok = bytes
                    .get(after)
                    .map(|b| !is_ident_byte(*b))
                    .unwrap_or(true);
                if after_ok {
                    return Some(name_start);
                }
            }
        }
    }
    None
}

/// Match `NAME:` written as a function parameter or struct field.
fn find_param_or_field(line: &str, name: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut pos = 0;
    while pos + name.len() < bytes.len() {
        let idx = match line[pos..].find(name) {
            Some(i) => pos + i,
            None => return None,
        };
        let after = idx + name.len();

        let left_ok = idx == 0 || !is_ident_byte(bytes[idx - 1]);
        let next = bytes.get(after).copied();
        let colon_after = next == Some(b':');
        // Reject e.g. `name::` (no such WGSL syntax today, but keeps us honest)
        let colon_isolated = colon_after && bytes.get(after + 1).copied() != Some(b':');

        if left_ok && colon_isolated {
            // Accept either:
            //   - struct field: only whitespace before `name:` on this line
            //   - parameter: `(` or `,` immediately before `name:` (allowing
            //     intervening whitespace)
            let before = line[..idx].trim_end();
            if before.is_empty() || before.ends_with('(') || before.ends_with(',') {
                return Some(idx);
            }
        }

        pos = idx + 1;
    }
    None
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_decl_basic() {
        assert_eq!(find_keyword_decl("fn foo() {}", "foo"), Some(3));
        assert_eq!(find_keyword_decl("struct Bar {}", "Bar"), Some(7));
        assert_eq!(find_keyword_decl("let x = 1;", "x"), Some(4));
        assert_eq!(find_keyword_decl("const PI: f32 = 3.0;", "PI"), Some(6));
    }

    #[test]
    fn keyword_decl_with_template() {
        assert_eq!(
            find_keyword_decl("var<uniform> globals: Foo;", "globals"),
            Some(13)
        );
        assert_eq!(
            find_keyword_decl("    var<storage, read> buf: Bar;", "buf"),
            Some(23)
        );
    }

    #[test]
    fn keyword_decl_rejects_partial_match() {
        // `fn foobar` should not match `foo`.
        assert_eq!(find_keyword_decl("fn foobar() {}", "foo"), None);
    }

    #[test]
    fn keyword_decl_with_attribute_prefix() {
        assert_eq!(
            find_keyword_decl("@fragment fn fs_main() {}", "fs_main"),
            Some(13)
        );
        assert_eq!(
            find_keyword_decl(
                "@group(0) @binding(0) var<uniform> globals: Foo;",
                "globals"
            ),
            Some(35)
        );
    }

    #[test]
    fn param_match() {
        assert_eq!(
            find_param_or_field("fn shade(pos: vec3f, dir: vec3f) {}", "pos"),
            Some(9)
        );
        assert_eq!(
            find_param_or_field("fn shade(pos: vec3f, dir: vec3f) {}", "dir"),
            Some(21)
        );
    }

    #[test]
    fn param_match_multiline() {
        // continuation line
        assert_eq!(find_param_or_field("    pos: vec3f,", "pos"), Some(4));
    }

    #[test]
    fn field_match() {
        assert_eq!(find_param_or_field("    origin: vec3f,", "origin"), Some(4));
    }

    #[test]
    fn param_field_does_not_match_let_with_type() {
        // `let foo: i32 = 0;` — `foo` is matched by the keyword path, not the
        // param/field path.
        assert_eq!(find_param_or_field("let foo: i32 = 0;", "foo"), None);
    }

    // End-to-end tests that exercise the workspace + preprocessor pipeline.
    mod e2e {
        use super::*;
        use std::fs;

        fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
            let path = dir.join(name);
            fs::write(&path, content).unwrap();
            path
        }

        fn open_in_workspace(workspace: &mut Workspace, path: &Path) -> String {
            let uri = path_to_uri(path);
            let text = fs::read_to_string(path).unwrap();
            workspace.update_document(&uri, text);
            uri
        }

        #[test]
        fn jump_into_included_header() {
            let dir = tempfile::tempdir().unwrap();
            let header = write_file(
                dir.path(),
                "header.wgsl",
                "#pragma once\nfn helper(x: f32) -> f32 { return x * 2.0; }\n",
            );
            let main = write_file(
                dir.path(),
                "main.wgsl",
                "#include \"header.wgsl\"\n@fragment fn fs_main() -> @location(0) vec4<f32> {\n    return vec4<f32>(helper(0.5), 0.0, 0.0, 1.0);\n}\n",
            );

            let mut ws = Workspace::new(dir.path().to_path_buf());
            let main_uri = open_in_workspace(&mut ws, &main);

            // Position cursor on `helper` call inside main.wgsl
            let pos = Position::new(2, 22);
            let loc = goto_definition(&ws, &main_uri, pos).expect("definition resolved");

            // The definition lives in header.wgsl, not main.wgsl. The
            // preprocessor canonicalizes paths (resolving symlinks), so
            // compare against the canonical URI rather than the literal one.
            let canonical_header = header.canonicalize().unwrap();
            assert_eq!(loc.uri.as_str(), path_to_uri(&canonical_header).as_str());
            assert_eq!(loc.range.start.line, 1); // 0-based line of `fn helper`
        }

        #[test]
        fn jump_to_function_parameter() {
            let dir = tempfile::tempdir().unwrap();
            let main = write_file(
                dir.path(),
                "main.wgsl",
                "fn shade(pos: vec3<f32>, dir: vec3<f32>) -> f32 {\n    return pos.x + dir.y;\n}\n",
            );

            let mut ws = Workspace::new(dir.path().to_path_buf());
            let uri = open_in_workspace(&mut ws, &main);

            // Cursor on `pos` inside the body (line 1, char 11)
            let pos = Position::new(1, 11);
            let loc = goto_definition(&ws, &uri, pos).expect("parameter resolved");
            assert_eq!(loc.uri.as_str(), uri.as_str());
            assert_eq!(loc.range.start.line, 0);
            // `pos` in `fn shade(pos:` lives at column 9
            assert_eq!(loc.range.start.character, 9);
        }

        #[test]
        fn jump_to_struct_field() {
            let dir = tempfile::tempdir().unwrap();
            let main = write_file(
                dir.path(),
                "main.wgsl",
                "struct Ray {\n    origin: vec3<f32>,\n    dir: vec3<f32>,\n}\nfn march(r: Ray) -> f32 { return r.origin.x; }\n",
            );

            let mut ws = Workspace::new(dir.path().to_path_buf());
            let uri = open_in_workspace(&mut ws, &main);

            // Cursor on `origin` inside `r.origin.x` (line 4, around column 36)
            let pos = Position::new(4, 36);
            let loc = goto_definition(&ws, &uri, pos).expect("field resolved");
            assert_eq!(loc.range.start.line, 1);
            assert_eq!(loc.range.start.character, 4);
        }
    }
}

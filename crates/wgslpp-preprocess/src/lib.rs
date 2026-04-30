pub mod conditional;
pub mod config;
pub mod evaluator;
pub mod include;
pub mod macros;
pub mod packages;
pub mod source_map;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use conditional::ConditionalStack;
use include::{parse_include, resolve_include};
use macros::{expand_macros, parse_define, MacroDef};
use packages::PackageRegistry;
use source_map::SourceMap;

/// Configuration for the preprocessor.
#[derive(Debug, Clone)]
pub struct PreprocessConfig {
    /// Named packages for `#include <pkg/path>`.
    pub packages: PackageRegistry,
    /// Initial `#define`s (from -D flags).
    pub defines: HashMap<String, String>,
}

impl Default for PreprocessConfig {
    fn default() -> Self {
        Self {
            packages: PackageRegistry::new(),
            defines: HashMap::new(),
        }
    }
}

/// Error from preprocessing.
#[derive(Debug, thiserror::Error)]
pub enum PreprocessError {
    #[error("{file}:{line}: {message}")]
    Directive {
        file: PathBuf,
        line: u32,
        message: String,
    },
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("circular include detected: {path}")]
    CircularInclude { path: PathBuf },
}

/// Result of preprocessing.
#[derive(Debug)]
pub struct PreprocessOutput {
    /// The preprocessed source code.
    pub code: String,
    /// Source map: output lines -> original locations.
    pub source_map: SourceMap,
    /// Final `#define` table after all directives have been processed.
    /// Object-macro values are stored as their literal text; flag-style
    /// `#define FOO` (no value) stores an empty string. Function-like macros
    /// also store an empty string here (the full definition lives elsewhere).
    pub defines: HashMap<String, String>,
}

/// Preprocess a WGSL file with `#include`, `#define`, `#ifdef`, etc.
pub fn preprocess(
    input_path: &Path,
    config: &PreprocessConfig,
) -> Result<PreprocessOutput, PreprocessError> {
    let mut ctx = PreprocessContext {
        config,
        source_map: SourceMap::new(),
        output_lines: Vec::new(),
        include_stack: HashSet::new(),
        pragma_once_files: HashSet::new(),
        // Convert simple defines to MacroDefs for macro expansion
        macro_defs: HashMap::new(),
        // Keep the simple defines map for conditional evaluation
        defines: config.defines.clone(),
    };

    // Seed macro_defs from config defines (object macros)
    for (name, value) in &config.defines {
        ctx.macro_defs
            .insert(name.clone(), MacroDef::Object(value.clone()));
    }

    let canonical = input_path.canonicalize().map_err(|e| PreprocessError::Io {
        path: input_path.to_path_buf(),
        source: e,
    })?;

    ctx.process_file(&canonical)?;

    let code = ctx.output_lines.join("\n");
    Ok(PreprocessOutput {
        code,
        source_map: ctx.source_map,
        defines: ctx.defines,
    })
}

/// Preprocess from a string (for testing or when the source isn't on disk).
pub fn preprocess_str(
    source: &str,
    file_name: &str,
    config: &PreprocessConfig,
) -> Result<PreprocessOutput, PreprocessError> {
    let mut ctx = PreprocessContext {
        config,
        source_map: SourceMap::new(),
        output_lines: Vec::new(),
        include_stack: HashSet::new(),
        pragma_once_files: HashSet::new(),
        macro_defs: HashMap::new(),
        defines: config.defines.clone(),
    };

    for (name, value) in &config.defines {
        ctx.macro_defs
            .insert(name.clone(), MacroDef::Object(value.clone()));
    }

    let file_path = PathBuf::from(file_name);
    ctx.process_source(source, &file_path)?;

    let code = ctx.output_lines.join("\n");
    Ok(PreprocessOutput {
        code,
        source_map: ctx.source_map,
        defines: ctx.defines,
    })
}

struct PreprocessContext<'a> {
    config: &'a PreprocessConfig,
    source_map: SourceMap,
    output_lines: Vec<String>,
    include_stack: HashSet<PathBuf>,
    pragma_once_files: HashSet<PathBuf>,
    macro_defs: HashMap<String, MacroDef>,
    defines: HashMap<String, String>,
}

impl<'a> PreprocessContext<'a> {
    fn process_file(&mut self, path: &Path) -> Result<(), PreprocessError> {
        if self.pragma_once_files.contains(path) {
            return Ok(());
        }

        if self.include_stack.contains(path) {
            return Err(PreprocessError::CircularInclude {
                path: path.to_path_buf(),
            });
        }

        let source = std::fs::read_to_string(path).map_err(|e| PreprocessError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        self.include_stack.insert(path.to_path_buf());
        self.process_source(&source, path)?;
        self.include_stack.remove(path);

        Ok(())
    }

    fn process_source(&mut self, source: &str, file_path: &Path) -> Result<(), PreprocessError> {
        let file_idx = self.source_map.intern_file(file_path);
        let mut cond_stack = ConditionalStack::new();

        for (line_num_0, line) in source.lines().enumerate() {
            let line_num = (line_num_0 + 1) as u32;
            let trimmed = line.trim();

            if let Some(directive) = trimmed.strip_prefix('#') {
                let directive = directive.trim();

                // These directives are always processed (even in inactive blocks)
                // for proper nesting tracking
                if let Some(rest) = strip_directive(directive, "ifdef") {
                    if cond_stack.is_active() {
                        cond_stack.ifdef(rest.trim(), &self.defines);
                    } else {
                        // Still need to track nesting
                        cond_stack.ifdef("__NEVER_DEFINED__", &self.defines);
                    }
                    continue;
                }
                if let Some(rest) = strip_directive(directive, "ifndef") {
                    if cond_stack.is_active() {
                        cond_stack.ifndef(rest.trim(), &self.defines);
                    } else {
                        cond_stack.ifdef("__NEVER_DEFINED__", &self.defines);
                    }
                    continue;
                }
                if let Some(rest) = strip_directive(directive, "if") {
                    if cond_stack.is_active() {
                        cond_stack
                            .if_expr(rest.trim(), &self.defines)
                            .map_err(|message| PreprocessError::Directive {
                                file: file_path.to_path_buf(),
                                line: line_num,
                                message,
                            })?;
                    } else {
                        cond_stack.ifdef("__NEVER_DEFINED__", &self.defines);
                    }
                    continue;
                }
                if let Some(rest) = strip_directive(directive, "elif") {
                    cond_stack
                        .elif(rest.trim(), &self.defines)
                        .map_err(|message| PreprocessError::Directive {
                            file: file_path.to_path_buf(),
                            line: line_num,
                            message,
                        })?;
                    continue;
                }
                if directive == "else" || directive.starts_with("else ") {
                    cond_stack
                        .else_branch()
                        .map_err(|message| PreprocessError::Directive {
                            file: file_path.to_path_buf(),
                            line: line_num,
                            message,
                        })?;
                    continue;
                }
                if directive == "endif" || directive.starts_with("endif ") {
                    cond_stack
                        .endif()
                        .map_err(|message| PreprocessError::Directive {
                            file: file_path.to_path_buf(),
                            line: line_num,
                            message,
                        })?;
                    continue;
                }

                // Remaining directives only processed when active
                if !cond_stack.is_active() {
                    continue;
                }

                if let Some(rest) = strip_directive(directive, "pragma") {
                    if rest.trim() == "once" {
                        self.pragma_once_files.insert(file_path.to_path_buf());
                        continue;
                    }
                    // Unknown pragma — fall through to emit as-is
                }

                if let Some(rest) = strip_directive(directive, "include") {
                    let inc_kind =
                        parse_include(rest.trim()).map_err(|message| {
                            PreprocessError::Directive {
                                file: file_path.to_path_buf(),
                                line: line_num,
                                message,
                            }
                        })?;
                    let resolved = resolve_include(&inc_kind, file_path, &self.config.packages)
                        .map_err(|message| PreprocessError::Directive {
                            file: file_path.to_path_buf(),
                            line: line_num,
                            message,
                        })?;
                    let canonical =
                        resolved
                            .canonicalize()
                            .map_err(|e| PreprocessError::Io {
                                path: resolved.clone(),
                                source: e,
                            })?;
                    self.process_file(&canonical)?;
                    continue;
                }

                if let Some(rest) = strip_directive(directive, "define") {
                    let (name, macro_def) =
                        parse_define(rest.trim()).map_err(|message| {
                            PreprocessError::Directive {
                                file: file_path.to_path_buf(),
                                line: line_num,
                                message,
                            }
                        })?;
                    // Update the simple defines map for conditionals
                    match &macro_def {
                        MacroDef::Object(val) => {
                            self.defines.insert(name.clone(), val.clone());
                        }
                        MacroDef::Function { .. } => {
                            // Function macros are "defined" for #ifdef purposes
                            self.defines.insert(name.clone(), String::new());
                        }
                    }
                    self.macro_defs.insert(name, macro_def);
                    continue;
                }

                if let Some(rest) = strip_directive(directive, "undef") {
                    let name = rest.trim();
                    self.defines.remove(name);
                    self.macro_defs.remove(name);
                    continue;
                }

                // Unknown directive — emit as-is (could be a WGSL attribute or comment starting with #)
                // Fall through to normal line emission
            }

            if !cond_stack.is_active() {
                continue;
            }

            // Expand macros in the line
            let expanded = if self.macro_defs.is_empty() {
                line.to_string()
            } else {
                expand_macros(line, &self.macro_defs)
            };

            self.source_map.push(file_idx, line_num);
            self.output_lines.push(expanded);
        }

        cond_stack
            .check_balanced()
            .map_err(|message| PreprocessError::Directive {
                file: file_path.to_path_buf(),
                line: source.lines().count() as u32,
                message,
            })?;

        Ok(())
    }
}

/// Strip a directive keyword from the start of a directive string.
/// Returns the remainder if the directive matches.
fn strip_directive<'a>(directive: &'a str, keyword: &str) -> Option<&'a str> {
    if directive == keyword {
        Some("")
    } else if let Some(rest) = directive.strip_prefix(keyword) {
        if rest.starts_with(' ') || rest.starts_with('\t') {
            Some(rest)
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preprocess_test(source: &str, defines: &[(&str, &str)]) -> Result<String, PreprocessError> {
        let mut config = PreprocessConfig::default();
        for (k, v) in defines {
            config.defines.insert(k.to_string(), v.to_string());
        }
        let out = preprocess_str(source, "test.wgsl", &config)?;
        Ok(out.code)
    }

    #[test]
    fn test_passthrough() {
        let code = preprocess_test("let x = 1;\nlet y = 2;", &[]).unwrap();
        assert_eq!(code, "let x = 1;\nlet y = 2;");
    }

    #[test]
    fn test_ifdef_defined() {
        let code = preprocess_test(
            "#ifdef FOO\nlet x = 1;\n#endif\nlet y = 2;",
            &[("FOO", "")],
        )
        .unwrap();
        assert_eq!(code, "let x = 1;\nlet y = 2;");
    }

    #[test]
    fn test_ifdef_undefined() {
        let code = preprocess_test("#ifdef FOO\nlet x = 1;\n#endif\nlet y = 2;", &[]).unwrap();
        assert_eq!(code, "let y = 2;");
    }

    #[test]
    fn test_ifndef() {
        let code = preprocess_test("#ifndef FOO\nlet x = 1;\n#endif", &[]).unwrap();
        assert_eq!(code, "let x = 1;");
    }

    #[test]
    fn test_if_elif_else() {
        let source = "#if X == 1\nA\n#elif X == 2\nB\n#else\nC\n#endif";
        assert_eq!(preprocess_test(source, &[("X", "1")]).unwrap(), "A");
        assert_eq!(preprocess_test(source, &[("X", "2")]).unwrap(), "B");
        assert_eq!(preprocess_test(source, &[("X", "3")]).unwrap(), "C");
    }

    #[test]
    fn test_define_and_use() {
        let code = preprocess_test("#define PI 3.14\nlet x = PI;", &[]).unwrap();
        assert_eq!(code, "let x = 3.14;");
    }

    #[test]
    fn test_undef() {
        let code = preprocess_test(
            "#define FOO 1\n#ifdef FOO\nA\n#endif\n#undef FOO\n#ifdef FOO\nB\n#endif",
            &[],
        )
        .unwrap();
        assert_eq!(code, "A");
    }

    #[test]
    fn test_nested_ifdef() {
        let code = preprocess_test(
            "#ifdef A\n#ifdef B\nAB\n#else\nA_ONLY\n#endif\n#endif",
            &[("A", "")],
        )
        .unwrap();
        assert_eq!(code, "A_ONLY");
    }

    #[test]
    fn test_source_map() {
        let config = PreprocessConfig::default();
        let out =
            preprocess_str("line1\n#ifdef NOPE\nskipped\n#endif\nline5", "test.wgsl", &config)
                .unwrap();
        assert_eq!(out.code, "line1\nline5");
        // line1 -> test.wgsl:1, line5 -> test.wgsl:5
        let (f0, l0) = out.source_map.lookup(0).unwrap();
        assert_eq!(f0, Path::new("test.wgsl"));
        assert_eq!(l0, 1);
        let (f1, l1) = out.source_map.lookup(1).unwrap();
        assert_eq!(f1, Path::new("test.wgsl"));
        assert_eq!(l1, 5);
    }

    #[test]
    fn test_pragma_once() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("header.wgsl"),
            "#pragma once\nconst PI = 3.14;",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("main.wgsl"),
            "#include \"header.wgsl\"\n#include \"header.wgsl\"\nlet x = PI;",
        )
        .unwrap();

        let config = PreprocessConfig::default();
        let result = preprocess(&temp_dir.path().join("main.wgsl"), &config).unwrap();
        // PI should appear exactly once despite two includes
        let pi_count = result.code.matches("const PI").count();
        assert_eq!(pi_count, 1, "pragma once should prevent duplicate: {}", result.code);
    }

    #[test]
    fn test_include_multiple_times_without_pragma_once() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("repeated.wgsl"),
            "// no pragma once\nconst VAL = 1;",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("main.wgsl"),
            "#include \"repeated.wgsl\"\n#include \"repeated.wgsl\"",
        )
        .unwrap();

        let config = PreprocessConfig::default();
        let result = preprocess(&temp_dir.path().join("main.wgsl"), &config).unwrap();
        // Should appear twice — no pragma once
        let val_count = result.code.matches("const VAL").count();
        assert_eq!(val_count, 2, "without pragma once, include should repeat: {}", result.code);
    }

    #[test]
    fn test_pragma_once_transitive() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("common.wgsl"),
            "#pragma once\nconst COMMON = 1;",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("a.wgsl"),
            "#include \"common.wgsl\"\nconst A = COMMON;",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("b.wgsl"),
            "#include \"common.wgsl\"\nconst B = COMMON;",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("main.wgsl"),
            "#include \"a.wgsl\"\n#include \"b.wgsl\"",
        )
        .unwrap();

        let config = PreprocessConfig::default();
        let result = preprocess(&temp_dir.path().join("main.wgsl"), &config).unwrap();
        // common.wgsl included via a.wgsl and b.wgsl, but should appear only once
        let count = result.code.matches("const COMMON").count();
        assert_eq!(count, 1, "pragma once should work transitively: {}", result.code);
        // But A and B should both appear
        assert!(result.code.contains("const A"), "a.wgsl content should appear");
        assert!(result.code.contains("const B"), "b.wgsl content should appear");
    }

    #[test]
    fn test_unbalanced_endif() {
        let result = preprocess_test("#endif", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_unclosed_if() {
        let result = preprocess_test("#ifdef FOO\nhello", &[("FOO", "")]);
        assert!(result.is_err());
    }
}

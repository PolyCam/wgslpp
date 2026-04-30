use naga::front::wgsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::WithSpan;
use wgslpp_preprocess::source_map::SourceMap;

/// A validation diagnostic with location info mapped back to the original source.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    /// Original file path (if source map available).
    pub file: Option<String>,
    /// 1-based line number in original source.
    pub line: Option<u32>,
    /// 1-based column (if available from naga).
    pub column: Option<u32>,
    /// Additional notes/labels from naga.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// Result of validation: a parsed+validated naga module (if successful) plus any diagnostics.
pub struct ValidationResult {
    pub module: Option<naga::Module>,
    pub module_info: Option<naga::valid::ModuleInfo>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse and validate WGSL source code.
/// Uses the source map to remap error locations to original file positions.
pub fn validate(source: &str, source_map: Option<&SourceMap>) -> ValidationResult {
    // Parse with `parse_doc_comments: true` so `module.doc_comments` is
    // populated. Reflection reads marker comments (`/// @unfilterable` etc.)
    // off this map rather than re-scanning the source.
    let mut frontend = wgsl::Frontend::new_with_options(wgsl::Options {
        parse_doc_comments: true,
    });
    let module = match frontend.parse(source) {
        Ok(module) => module,
        Err(parse_error) => {
            let diagnostics = parse_error_to_diagnostics(&parse_error, source, source_map);
            return ValidationResult {
                module: None,
                module_info: None,
                diagnostics,
            };
        }
    };

    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    match validator.validate(&module) {
        Ok(module_info) => ValidationResult {
            module: Some(module),
            module_info: Some(module_info),
            diagnostics: Vec::new(),
        },
        Err(validation_error) => {
            let diagnostics =
                validation_error_to_diagnostics(&validation_error, source, source_map);
            ValidationResult {
                module: Some(module),
                module_info: None,
                diagnostics,
            }
        }
    }
}

/// Convert a naga parse error into diagnostics.
fn parse_error_to_diagnostics(
    error: &wgsl::ParseError,
    source: &str,
    source_map: Option<&SourceMap>,
) -> Vec<Diagnostic> {
    let message = error.emit_to_string(source);

    // Try to extract location from the error's labels
    let (file, line, column) = extract_error_location(error, source, source_map);

    vec![Diagnostic {
        severity: Severity::Error,
        message: message.trim().to_string(),
        file,
        line,
        column,
        notes: Vec::new(),
    }]
}

/// Extract location from a naga ParseError by examining the formatted output.
fn extract_error_location(
    error: &wgsl::ParseError,
    source: &str,
    source_map: Option<&SourceMap>,
) -> (Option<String>, Option<u32>, Option<u32>) {
    // naga ParseError has labels with spans - use the message to find the location
    let msg = error.emit_to_string(source);

    // Parse the error output to find line numbers
    // naga formats as: "error: ... \n   ┌─ wgsl:LINE:COL"
    for line in msg.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("┌─ wgsl:") {
            let parts: Vec<&str> = rest.split(':').collect();
            if let Some(line_str) = parts.first() {
                if let Ok(output_line) = line_str.parse::<u32>() {
                    let col = parts.get(1).and_then(|s| s.parse::<u32>().ok());
                    // Map through source map
                    if let Some(sm) = source_map {
                        if let Some((file, src_line)) =
                            sm.lookup((output_line - 1) as usize)
                        {
                            return (
                                Some(file.to_string_lossy().to_string()),
                                Some(src_line),
                                col,
                            );
                        }
                    }
                    return (None, Some(output_line), col);
                }
            }
        }
    }

    (None, None, None)
}

/// Convert a naga validation error (wrapped in WithSpan) into diagnostics.
fn validation_error_to_diagnostics(
    error: &WithSpan<naga::valid::ValidationError>,
    source: &str,
    source_map: Option<&SourceMap>,
) -> Vec<Diagnostic> {
    let full_message = format!("{}", error.as_inner());
    let mut notes = Vec::new();

    // Walk the error chain for additional context
    let mut current: &dyn std::error::Error = error.as_inner();
    while let Some(cause) = current.source() {
        notes.push(format!("{}", cause));
        current = cause;
    }

    // Try to find location from the WithSpan's spans
    let (file, line, column) = find_validation_error_location(error, source, source_map);

    vec![Diagnostic {
        severity: Severity::Error,
        message: full_message,
        file,
        line,
        column,
        notes,
    }]
}

fn find_validation_error_location(
    error: &WithSpan<naga::valid::ValidationError>,
    source: &str,
    source_map: Option<&SourceMap>,
) -> (Option<String>, Option<u32>, Option<u32>) {
    // Try to get the first span from the error
    for (span, _) in error.spans() {
        if !span.is_defined() {
            continue;
        }
        let loc = span.location(source);
        let output_line = (loc.line_number - 1) as usize;
        let column = loc.line_position;

        if let Some(sm) = source_map {
            if let Some((file, src_line)) = sm.lookup(output_line) {
                return (
                    Some(file.to_string_lossy().to_string()),
                    Some(src_line),
                    Some(column),
                );
            }
        }
        return (None, Some(loc.line_number), Some(column));
    }
    (None, None, None)
}

/// Format diagnostics for human-readable terminal output.
pub fn format_diagnostics_human(diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();
    for diag in diagnostics {
        let severity_str = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let location = match (&diag.file, diag.line) {
            (Some(file), Some(line)) => {
                if let Some(col) = diag.column {
                    format!("{}:{}:{}: ", file, line, col)
                } else {
                    format!("{}:{}: ", file, line)
                }
            }
            (None, Some(line)) => format!(":{}: ", line),
            _ => String::new(),
        };
        output.push_str(&format!("{}{}: {}\n", location, severity_str, diag.message));
        for note in &diag.notes {
            output.push_str(&format!("  note: {}\n", note));
        }
    }
    output
}

/// Format diagnostics as GCC-style output (file:line:col: severity: message).
pub fn format_diagnostics_gcc(diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();
    for diag in diagnostics {
        let severity_str = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let file = diag.file.as_deref().unwrap_or("<unknown>");
        let line = diag.line.unwrap_or(0);
        let col = diag.column.unwrap_or(0);
        output.push_str(&format!(
            "{}:{}:{}: {}: {}\n",
            file, line, col, severity_str, diag.message
        ));
    }
    output
}

/// Format diagnostics as JSON.
pub fn format_diagnostics_json(diagnostics: &[Diagnostic]) -> String {
    serde_json::to_string_pretty(diagnostics).unwrap_or_else(|_| "[]".to_string())
}

use std::path::PathBuf;
use wgslpp_core::validate::{
    format_diagnostics_gcc, format_diagnostics_human, format_diagnostics_json, validate,
};
use wgslpp_preprocess::source_map::SourceMap;

use crate::DiagnosticFormat;

pub fn run(
    input: PathBuf,
    source_map_path: Option<PathBuf>,
    format: DiagnosticFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(&input)?;

    let source_map = if let Some(sm_path) = source_map_path {
        let sm_json = std::fs::read_to_string(&sm_path)?;
        Some(serde_json::from_str::<SourceMap>(&sm_json)?)
    } else {
        None
    };

    let result = validate(&source, source_map.as_ref());

    if result.diagnostics.is_empty() {
        eprintln!("Valid.");
        Ok(())
    } else {
        let output = match format {
            DiagnosticFormat::Human => format_diagnostics_human(&result.diagnostics),
            DiagnosticFormat::Json => format_diagnostics_json(&result.diagnostics),
            DiagnosticFormat::Gcc => format_diagnostics_gcc(&result.diagnostics),
        };
        eprint!("{}", output);
        if result.diagnostics.iter().any(|d| d.severity == wgslpp_core::validate::Severity::Error) {
            Err("validation failed".into())
        } else {
            Ok(())
        }
    }
}

use std::io::Read;
use std::path::PathBuf;
use wgslpp_core::attributes::extract_attributes;
use wgslpp_core::reflect::reflect;
use wgslpp_core::validate::{format_diagnostics_human, validate};

pub fn run(
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    use_stdin: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = if use_stdin {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        let path = input.ok_or("input file required (or use --stdin)")?;
        std::fs::read_to_string(&path)?
    };

    let result = validate(&source, None);

    let module = match result.module {
        Some(m) => m,
        None => {
            let msg = if result.diagnostics.is_empty() {
                "parse failed — cannot reflect".to_string()
            } else {
                format!(
                    "parse failed — cannot reflect:\n{}",
                    format_diagnostics_human(&result.diagnostics)
                )
            };
            return Err(msg.into());
        }
    };

    if result.module_info.is_none() && !result.diagnostics.is_empty() {
        let label = if use_stdin { "<stdin>" } else { "input" };
        eprintln!(
            "warning: {} has validation errors, reflecting from parsed module:\n{}",
            label,
            format_diagnostics_human(&result.diagnostics)
        );
    }

    let attributes = extract_attributes(&module);
    let reflection = reflect(&module, &attributes);
    let json = serde_json::to_string_pretty(&reflection)?;

    match output {
        Some(path) => std::fs::write(&path, json)?,
        None => println!("{}", json),
    }

    Ok(())
}

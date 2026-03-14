use std::path::PathBuf;
use wgslpp_core::minify::minify;
use wgslpp_core::reflect::reflect;
use wgslpp_core::validate::validate;
use wgslpp_preprocess::packages::PackageRegistry;
use wgslpp_preprocess::{preprocess, PreprocessConfig};

pub fn run(
    input: PathBuf,
    packages: Vec<(String, PathBuf)>,
    defines: Vec<(String, String)>,
    output: Option<PathBuf>,
    reflect_path: Option<PathBuf>,
    source_map_path: Option<PathBuf>,
    no_validate: bool,
    no_minify: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Preprocess
    let mut registry = PackageRegistry::new();
    for (name, path) in packages {
        registry.add(name, path);
    }

    let config = PreprocessConfig {
        packages: registry,
        defines: defines.into_iter().collect(),
    };

    let pp_result = preprocess(&input, &config)?;

    // Write source map if requested
    if let Some(sm_path) = source_map_path {
        let json = serde_json::to_string_pretty(&pp_result.source_map.to_json())?;
        std::fs::write(&sm_path, json)?;
    }

    // 2. Validate (unless --no-validate)
    let validation_result = validate(&pp_result.code, Some(&pp_result.source_map));

    if !no_validate && !validation_result.diagnostics.is_empty() {
        let output =
            wgslpp_core::validate::format_diagnostics_human(&validation_result.diagnostics);
        eprint!("{}", output);
        if validation_result
            .diagnostics
            .iter()
            .any(|d| d.severity == wgslpp_core::validate::Severity::Error)
        {
            return Err("validation failed".into());
        }
    }

    // 3. Reflect (if requested)
    if let Some(ref_path) = reflect_path {
        if let (Some(module), Some(module_info)) =
            (&validation_result.module, &validation_result.module_info)
        {
            let reflection = reflect(module, module_info);
            let json = serde_json::to_string_pretty(&reflection)?;
            std::fs::write(&ref_path, json)?;
        } else if !no_validate {
            return Err("cannot reflect: validation failed".into());
        }
    }

    // 4. Output (optionally minified)
    let final_code = if !no_minify {
        if let (Some(module), Some(module_info)) =
            (&validation_result.module, &validation_result.module_info)
        {
            match minify(module, module_info) {
                Ok(minified) => minified,
                Err(e) => {
                    eprintln!("warning: minification failed ({}), using preprocessed output", e);
                    pp_result.code
                }
            }
        } else {
            pp_result.code
        }
    } else {
        pp_result.code
    };

    match output {
        Some(path) => std::fs::write(&path, &final_code)?,
        None => print!("{}", final_code),
    }

    Ok(())
}

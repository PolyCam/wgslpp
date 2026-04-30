//! `wgslpp pipeline` — preprocess + validate + reflect (+ optional minify) in
//! one invocation. Replaces orchestrating multiple wgslpp calls from build
//! scripts: a single subprocess returns code, defines, and reflection at once.

use serde_json::json;
use std::path::PathBuf;
use wgslpp_core::attributes::extract_attributes;
use wgslpp_core::dce::eliminate_dead_code;
use wgslpp_core::minify::minify;
use wgslpp_core::reflect::reflect;
use wgslpp_core::rename::rename_identifiers;
use wgslpp_core::validate::{format_diagnostics_human, validate, Severity};
use wgslpp_preprocess::config::WgslppConfig;
use wgslpp_preprocess::packages::PackageRegistry;
use wgslpp_preprocess::{preprocess, PreprocessConfig};

pub fn run(
    input: PathBuf,
    config_path: Option<PathBuf>,
    packages: Vec<(String, PathBuf)>,
    defines: Vec<(String, String)>,
    output: Option<PathBuf>,
    source_map_path: Option<PathBuf>,
    no_validate: bool,
    minify_flag: bool,
    dce: bool,
    rename: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Preprocess (config → -P overlay → -D defines).
    let mut registry = if let Some(ref cfg_path) = config_path {
        let cfg = WgslppConfig::load(cfg_path)?;
        let base_dir = cfg_path.parent().unwrap_or_else(|| std::path::Path::new("."));
        cfg.to_packages(base_dir)
    } else {
        PackageRegistry::new()
    };
    for (name, path) in packages {
        registry.add(name, path);
    }
    let preprocess_config = PreprocessConfig {
        packages: registry,
        defines: defines.into_iter().collect(),
    };
    let pp = preprocess(&input, &preprocess_config)?;

    if let Some(sm_path) = source_map_path {
        let json = serde_json::to_string_pretty(&pp.source_map.to_json())?;
        std::fs::write(&sm_path, json)?;
    }

    // 2. Validate. The frontend is configured to retain `///` doc comments on
    // the parsed module, so attribute extraction below can read markers off
    // `module.doc_comments` rather than re-scanning the source.
    let validation = validate(&pp.code, Some(&pp.source_map));
    if !no_validate
        && validation
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    {
        eprint!("{}", format_diagnostics_human(&validation.diagnostics));
        return Err("validation failed".into());
    }

    let module = validation
        .module
        .ok_or_else(|| -> Box<dyn std::error::Error> { "parse failed — cannot reflect".into() })?;
    let module_info = validation.module_info;

    // 3. Reflect (on the parsed, pre-minify module so names are original).
    // Pull marker overrides off the module's doc-comment map.
    let attributes = extract_attributes(&module);
    let reflection = reflect(&module, &attributes);

    // 4. Optionally minify the code embedded in the JSON output.
    let final_code = if minify_flag {
        let info = module_info
            .as_ref()
            .ok_or_else(|| -> Box<dyn std::error::Error> {
                "validation failed — cannot minify".into()
            })?;
        let mut module = module.clone();
        if dce {
            eliminate_dead_code(&mut module);
        }
        if rename {
            rename_identifiers(&mut module);
        }
        match minify(&module, info) {
            Ok(minified) => minified,
            Err(e) => {
                eprintln!(
                    "warning: minification failed ({}), falling back to preprocessed source",
                    e
                );
                pp.code.clone()
            }
        }
    } else {
        pp.code.clone()
    };

    let json = serde_json::to_string(&json!({
        "code": final_code,
        "defines": pp.defines,
        "reflection": reflection,
    }))?;

    match output {
        Some(path) => std::fs::write(&path, json)?,
        None => print!("{}", json),
    }
    Ok(())
}

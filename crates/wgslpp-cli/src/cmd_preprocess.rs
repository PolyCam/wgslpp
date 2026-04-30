use std::io::Read;
use std::path::PathBuf;
use wgslpp_preprocess::config::WgslppConfig;
use wgslpp_preprocess::packages::PackageRegistry;
use wgslpp_preprocess::{preprocess, preprocess_str, PreprocessConfig};

pub fn run(
    input: Option<PathBuf>,
    packages: Vec<(String, PathBuf)>,
    defines: Vec<(String, String)>,
    output: Option<PathBuf>,
    source_map_path: Option<PathBuf>,
    config_path: Option<PathBuf>,
    use_stdin: bool,
    file_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build package registry: start from config, then overlay explicit -P flags
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

    let config = PreprocessConfig {
        packages: registry,
        defines: defines.into_iter().collect(),
    };

    let result = if use_stdin {
        let mut source = String::new();
        std::io::stdin().read_to_string(&mut source)?;
        let vpath = file_path
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<stdin>".to_string());
        preprocess_str(&source, &vpath, &config)?
    } else {
        let input = input.ok_or("input file required (or use --stdin)")?;
        preprocess(&input, &config)?
    };

    match output {
        Some(path) => std::fs::write(&path, &result.code)?,
        None => print!("{}", result.code),
    }

    if let Some(sm_path) = source_map_path {
        let json = serde_json::to_string_pretty(&result.source_map.to_json())?;
        std::fs::write(&sm_path, json)?;
    }

    Ok(())
}

use std::path::PathBuf;
use wgslpp_preprocess::{preprocess, PreprocessConfig};
use wgslpp_preprocess::packages::PackageRegistry;

pub fn run(
    input: PathBuf,
    packages: Vec<(String, PathBuf)>,
    defines: Vec<(String, String)>,
    output: Option<PathBuf>,
    source_map_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = PackageRegistry::new();
    for (name, path) in packages {
        registry.add(name, path);
    }

    let config = PreprocessConfig {
        packages: registry,
        defines: defines.into_iter().collect(),
    };

    let result = preprocess(&input, &config)?;

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

use std::path::PathBuf;
use wgslpp_core::dce::eliminate_dead_code;
use wgslpp_core::minify::minify;
use wgslpp_core::rename::rename_identifiers;
use wgslpp_core::validate::validate;

pub fn run(
    input: PathBuf,
    output: Option<PathBuf>,
    dce: bool,
    rename: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(&input)?;
    let result = validate(&source, None);

    let mut module = result.module.ok_or("parse failed — cannot minify")?;
    let module_info = result
        .module_info
        .ok_or("validation failed — cannot minify")?;

    if dce {
        eliminate_dead_code(&mut module);
    }

    if rename {
        rename_identifiers(&mut module);
    }

    let minified = minify(&module, &module_info)?;

    match output {
        Some(path) => std::fs::write(&path, &minified)?,
        None => print!("{}", minified),
    }

    Ok(())
}

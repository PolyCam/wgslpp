use std::path::PathBuf;
use wgslpp_core::reflect::reflect;
use wgslpp_core::validate::validate;

pub fn run(input: PathBuf, output: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string(&input)?;
    let result = validate(&source, None);

    let module = result
        .module
        .ok_or("parse failed — cannot reflect")?;
    let module_info = result
        .module_info
        .ok_or("validation failed — cannot reflect")?;

    let reflection = reflect(&module, &module_info);
    let json = serde_json::to_string_pretty(&reflection)?;

    match output {
        Some(path) => std::fs::write(&path, json)?,
        None => println!("{}", json),
    }

    Ok(())
}

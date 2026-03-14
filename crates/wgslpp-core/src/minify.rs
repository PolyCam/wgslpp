use naga::back::wgsl;
use naga::valid::ModuleInfo;

/// Minify a validated naga module by re-emitting it with the naga WGSL writer.
/// This strips comments and normalizes whitespace.
pub fn minify(module: &naga::Module, module_info: &ModuleInfo) -> Result<String, String> {
    let mut output = String::new();
    let mut writer = wgsl::Writer::new(&mut output, wgsl::WriterFlags::empty());
    writer
        .write(module, module_info)
        .map_err(|e| format!("naga WGSL writer error: {}", e))?;
    Ok(output)
}

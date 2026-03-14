use std::collections::HashMap;
use std::path::PathBuf;

use wgslpp_core::reflect::reflect;
use wgslpp_core::validate::validate;
use wgslpp_preprocess::packages::PackageRegistry;
use wgslpp_preprocess::{preprocess, preprocess_str, PreprocessConfig};

fn test_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = tests/integration/, so parent = tests/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

#[test]
fn test_validate_simple_shader() {
    let path = test_dir().join("integration/simple.wgsl");
    let source = std::fs::read_to_string(&path).unwrap();
    let result = validate(&source, None);
    assert!(
        result.diagnostics.is_empty(),
        "expected no errors, got: {:?}",
        result.diagnostics
    );
    assert!(result.module.is_some());
    assert!(result.module_info.is_some());
}

#[test]
fn test_reflect_simple_shader() {
    let path = test_dir().join("integration/simple.wgsl");
    let source = std::fs::read_to_string(&path).unwrap();
    let result = validate(&source, None);
    let module = result.module.unwrap();
    let info = result.module_info.unwrap();

    let reflection = reflect(&module, &info);

    assert_eq!(reflection.bindings.len(), 1);
    assert_eq!(reflection.bindings[0].group, 0);
    assert_eq!(reflection.bindings[0].binding, 0);
    assert_eq!(reflection.bindings[0].name.as_deref(), Some("mvp"));
    assert_eq!(reflection.bindings[0].binding_type, "uniform");

    assert_eq!(reflection.entry_points.len(), 2);
    assert_eq!(reflection.entry_points[0].name, "vs_main");
    assert_eq!(reflection.entry_points[0].stage, "vertex");
    assert_eq!(reflection.entry_points[1].name, "fs_main");
    assert_eq!(reflection.entry_points[1].stage, "fragment");

    // VertexOutput struct
    assert!(reflection.structs.iter().any(|s| s.name.as_deref() == Some("VertexOutput")));
}

#[test]
fn test_preprocess_with_includes() {
    let path = test_dir().join("preprocess/main.wgsl");
    let config = PreprocessConfig {
        packages: PackageRegistry::new(),
        defines: HashMap::new(),
    };
    let result = preprocess(&path, &config).unwrap();

    // Without USE_COLOR, should get the white color branch
    assert!(result.code.contains("vec3<f32>(1.0, 1.0, 1.0)"));
    assert!(!result.code.contains("BRAND_COLOR")); // BRAND_COLOR should be expanded
    assert!(result.code.contains("const PI")); // from common.wgsl
    assert!(result.code.contains("fn saturate")); // from common.wgsl
}

#[test]
fn test_preprocess_with_defines() {
    let path = test_dir().join("preprocess/main.wgsl");
    let mut defines = HashMap::new();
    defines.insert("USE_COLOR".to_string(), String::new());
    let config = PreprocessConfig {
        packages: PackageRegistry::new(),
        defines,
    };
    let result = preprocess(&path, &config).unwrap();

    // With USE_COLOR, should get the BRAND_COLOR branch (which gets macro-expanded)
    assert!(result.code.contains("vec3<f32>(0.2, 0.4, 0.8)"));
    assert!(!result.code.contains("vec3<f32>(1.0, 1.0, 1.0)"));
}

#[test]
fn test_preprocess_include_guard() {
    // common.wgsl uses include guards; including it twice should not duplicate content
    let source = "#include \"common.wgsl\"\n#include \"common.wgsl\"\nfn test() -> f32 { return PI; }";
    let config = PreprocessConfig::default();
    // Need to write a temp file that includes common.wgsl
    let temp_dir = tempfile::tempdir().unwrap();
    let common_src = std::fs::read_to_string(test_dir().join("preprocess/common.wgsl")).unwrap();
    std::fs::write(temp_dir.path().join("common.wgsl"), &common_src).unwrap();
    let main_path = temp_dir.path().join("main.wgsl");
    std::fs::write(&main_path, source).unwrap();

    let result = preprocess(&main_path, &config).unwrap();

    // PI should appear exactly once
    let pi_count = result.code.matches("const PI").count();
    assert_eq!(pi_count, 1, "include guard should prevent duplicate content");
}

#[test]
fn test_validate_detects_errors() {
    let source = r#"
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    let x: f32 = vec3<f32>(1.0, 0.0, 0.0); // type mismatch
    return vec4<f32>(1.0);
}
"#;
    let result = validate(source, None);
    assert!(!result.diagnostics.is_empty(), "should detect type error");
}

#[test]
fn test_source_map_error_remapping() {
    let path = test_dir().join("preprocess/main.wgsl");
    let config = PreprocessConfig {
        packages: PackageRegistry::new(),
        defines: HashMap::new(),
    };
    let pp = preprocess(&path, &config).unwrap();
    let result = validate(&pp.code, Some(&pp.source_map));
    // The preprocessed output should be valid WGSL, so no errors
    // (saturate is a valid user function name, and the code is correct)
    // This test mainly verifies source map is passed through without crashing
    let _ = result;
}

#[test]
fn test_full_pipeline_preprocess_validate_reflect() {
    let path = test_dir().join("preprocess/main.wgsl");
    let config = PreprocessConfig::default();
    let pp = preprocess(&path, &config).unwrap();

    // Validate the preprocessed output
    let validation = validate(&pp.code, Some(&pp.source_map));
    assert!(
        validation.diagnostics.is_empty(),
        "preprocessed shader should validate: {:?}",
        validation.diagnostics
    );

    // Reflect
    let module = validation.module.unwrap();
    let info = validation.module_info.unwrap();
    let reflection = reflect(&module, &info);

    assert_eq!(reflection.entry_points.len(), 1);
    assert_eq!(reflection.entry_points[0].name, "fs_main");
    assert_eq!(reflection.entry_points[0].stage, "fragment");
}

#[test]
fn test_circular_include_detection() {
    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("a.wgsl"),
        "#include \"b.wgsl\"\n",
    ).unwrap();
    std::fs::write(
        temp_dir.path().join("b.wgsl"),
        "#include \"a.wgsl\"\n",
    ).unwrap();

    let config = PreprocessConfig::default();
    let result = preprocess(&temp_dir.path().join("a.wgsl"), &config);
    assert!(result.is_err(), "should detect circular include");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("circular"), "error should mention circular: {}", err);
}

#[test]
fn test_function_like_macros() {
    let source = "#define LERP(a, b, t) mix(a, b, t)\n@fragment\nfn main() -> @location(0) vec4<f32> { return vec4<f32>(LERP(0.0, 1.0, 0.5)); }";
    let config = PreprocessConfig::default();
    let result = preprocess_str(source, "test.wgsl", &config).unwrap();
    assert!(result.code.contains("mix(0.0, 1.0, 0.5)"));
    assert!(!result.code.contains("LERP"));
}

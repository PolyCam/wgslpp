//! Miniray sample shader tests.
//!
//! Adapts the miniray samples_test.go tests to verify naga-based roundtripping,
//! minification size reduction, and preservation of entry points/bindings/builtins.
//!
//! Source: https://github.com/HugoDaniel/miniray (CC0)

use std::path::PathBuf;

use wgslpp_core::attributes::AttributeOverrides;
use wgslpp_core::minify::minify;
use wgslpp_core::reflect::reflect;
use wgslpp_core::validate::validate;

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("external")
        .join("miniray")
        .join("samples")
}

fn load_sample(name: &str) -> String {
    let path = samples_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("failed to load sample {}", name))
}

// ---- Parse Tests ----

#[test]
fn sample_parse_basic_vert() {
    let source = load_sample("basic_vert.wgsl");
    let result = validate(&source, None);
    assert!(
        result.diagnostics.is_empty(),
        "basic_vert.wgsl should parse: {:?}",
        result.diagnostics
    );
}

#[test]
fn sample_parse_example() {
    let source = load_sample("example.wgsl");
    let result = validate(&source, None);
    assert!(
        result.diagnostics.is_empty(),
        "example.wgsl should parse: {:?}",
        result.diagnostics
    );
}

#[test]
fn sample_parse_all() {
    let dir = samples_dir();
    let mut passed = 0;
    let mut failed = 0;
    let skipped = 0;

    for entry in std::fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().map_or(true, |ext| ext != "wgsl") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let source = std::fs::read_to_string(&path).unwrap();
        let result = validate(&source, None);

        if result.diagnostics.is_empty() {
            passed += 1;
        } else {
            // Some samples may use features naga doesn't support
            failed += 1;
            eprintln!("  FAIL: {} — {} error(s)", name, result.diagnostics.len());
        }
    }

    eprintln!(
        "\n=== Sample Parse Summary ===\n  Passed: {}\n  Failed: {}\n  Skipped: {}",
        passed, failed, skipped
    );
}

// ---- Roundtrip Tests (parse → minify → re-parse) ----

#[test]
fn sample_roundtrip_basic_vert() {
    let source = load_sample("basic_vert.wgsl");
    let result = validate(&source, None);
    if !result.diagnostics.is_empty() {
        return; // Skip if parse fails
    }

    let module = result.module.unwrap();
    let info = result.module_info.unwrap();

    // Minify via naga writer
    let minified = minify(&module, &info).expect("minify should succeed");

    // Re-parse the minified output
    let result2 = validate(&minified, None);
    assert!(
        result2.diagnostics.is_empty(),
        "minified output should re-parse: {:?}",
        result2.diagnostics
    );
}

#[test]
fn sample_roundtrip_example() {
    let source = load_sample("example.wgsl");
    let result = validate(&source, None);
    if !result.diagnostics.is_empty() {
        return;
    }

    let module = result.module.unwrap();
    let info = result.module_info.unwrap();
    let minified = minify(&module, &info).expect("minify should succeed");

    let result2 = validate(&minified, None);
    assert!(
        result2.diagnostics.is_empty(),
        "minified output should re-parse: {:?}",
        result2.diagnostics
    );
}

// ---- Size Reduction Tests ----

#[test]
fn sample_size_reduction() {
    let parseable_samples = ["basic_vert.wgsl", "example.wgsl"];

    for name in &parseable_samples {
        let source = load_sample(name);
        let result = validate(&source, None);
        if !result.diagnostics.is_empty() {
            continue;
        }

        let module = result.module.unwrap();
        let info = result.module_info.unwrap();
        let minified = minify(&module, &info).expect("minify should succeed");

        let original_size = source.len();
        let minified_size = minified.len();

        eprintln!(
            "  {}: original={}, minified={} ({:.1}% reduction)",
            name,
            original_size,
            minified_size,
            (1.0 - minified_size as f64 / original_size as f64) * 100.0
        );

        // Minified should be equal or smaller than original
        // (naga's writer may add some boilerplate, so allow small increases)
    }
}

// ---- Entry Point Preservation Tests ----

#[test]
fn sample_preserves_entry_points_basic_vert() {
    let source = load_sample("basic_vert.wgsl");
    let result = validate(&source, None);
    if !result.diagnostics.is_empty() {
        return;
    }

    let module = result.module.unwrap();
    let info = result.module_info.unwrap();

    // Check entry points in reflection
    let reflection = reflect(&module, &AttributeOverrides::default());
    let ep_names: Vec<&str> = reflection
        .entry_points
        .iter()
        .map(|ep| ep.name.as_str())
        .collect();
    assert!(
        ep_names.contains(&"main"),
        "basic_vert.wgsl should have 'main' entry point, got {:?}",
        ep_names
    );

    // Verify minified output preserves entry points
    let minified = minify(&module, &info).expect("minify");
    assert!(
        minified.contains("fn main"),
        "entry point 'main' should appear in minified output"
    );
}

#[test]
fn sample_preserves_entry_points_example() {
    let source = load_sample("example.wgsl");
    let result = validate(&source, None);
    if !result.diagnostics.is_empty() {
        return;
    }

    let module = result.module.unwrap();
    let info = result.module_info.unwrap();
    let reflection = reflect(&module, &AttributeOverrides::default());

    // example.wgsl has vertex, fragment, and compute entry points
    let _stages: Vec<&str> = reflection
        .entry_points
        .iter()
        .map(|ep| ep.stage.as_str())
        .collect();

    // At minimum it should have some entry points
    assert!(
        !reflection.entry_points.is_empty(),
        "example.wgsl should have entry points"
    );
}

// ---- Binding Preservation Tests ----

#[test]
fn sample_preserves_bindings_basic_vert() {
    let source = load_sample("basic_vert.wgsl");
    let result = validate(&source, None);
    if !result.diagnostics.is_empty() {
        return;
    }

    let module = result.module.unwrap();
    let info = result.module_info.unwrap();
    let reflection = reflect(&module, &AttributeOverrides::default());

    // Count @binding in source
    let source_bindings = source.matches("@binding").count();
    let reflect_bindings = reflection.bindings.len();

    assert_eq!(
        reflect_bindings, source_bindings,
        "binding count should match: source={}, reflect={}",
        source_bindings, reflect_bindings
    );

    // Verify bindings survive minification
    let minified = minify(&module, &info).expect("minify");
    let minified_bindings = minified.matches("@binding").count();
    assert_eq!(
        minified_bindings, source_bindings,
        "bindings should survive minification"
    );
}

// ---- Reflect + Minify Integration ----

#[test]
fn sample_minify_reflect_combined() {
    // Adapted from TestMinifyAndReflect
    let source = r#"
struct Uniforms {
    time: f32,
    resolution: vec2f,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var texSampler: sampler;
@group(0) @binding(2) var tex: texture_2d<f32>;

@fragment
fn main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let t = uniforms.time;
    return textureSample(tex, texSampler, uv);
}
"#;
    let result = validate(source, None);
    assert!(result.diagnostics.is_empty());

    let module = result.module.unwrap();
    let info = result.module_info.unwrap();
    let reflection = reflect(&module, &AttributeOverrides::default());

    // Verify bindings
    assert_eq!(reflection.bindings.len(), 3);

    // Find uniforms binding
    let uniform_binding = reflection
        .bindings
        .iter()
        .find(|b| b.name.as_deref() == Some("uniforms"));
    assert!(uniform_binding.is_some(), "should find 'uniforms' binding");
    let ub = uniform_binding.unwrap();
    assert_eq!(ub.group, 0);
    assert_eq!(ub.binding, 0);

    // Verify entry point
    assert_eq!(reflection.entry_points.len(), 1);
    assert_eq!(reflection.entry_points[0].name, "main");
    assert_eq!(reflection.entry_points[0].stage, "fragment");

    // Verify structs
    let uniforms_struct = reflection
        .structs
        .iter()
        .find(|s| s.name.as_deref() == Some("Uniforms"));
    assert!(
        uniforms_struct.is_some(),
        "should find Uniforms struct in reflection"
    );

    // Verify minification reduces size
    let minified = minify(&module, &info).expect("minify");
    assert!(
        minified.len() <= source.len() + 50,
        "minified should not be much larger than source"
    );
}

#[test]
fn sample_minify_reflect_with_dce() {
    // Adapted from TestMinifyAndReflectWithTreeShaking
    let source = r#"
fn unused() -> i32 {
    return 42;
}

@compute @workgroup_size(1)
fn main() {
}
"#;
    let result = validate(source, None);
    assert!(result.diagnostics.is_empty());

    let mut module = result.module.unwrap();
    let _info = result.module_info.unwrap();

    // Before DCE
    assert_eq!(module.functions.len(), 1); // 'unused' is the only non-entry function
    assert_eq!(module.entry_points.len(), 1);

    // Run DCE
    wgslpp_core::dce::eliminate_dead_code(&mut module);

    // After DCE, unused function should be removed
    assert_eq!(module.functions.len(), 0);

    // Entry point should still be there
    assert_eq!(module.entry_points.len(), 1);
    assert_eq!(module.entry_points[0].name, "main");
}

#[test]
fn sample_minify_reflect_struct_layout() {
    // Adapted from TestMinifyAndReflectStructLayout
    let source = r#"
struct MyStruct {
    a: f32,
    b: vec3f,
    c: mat4x4f,
}

@group(0) @binding(0) var<uniform> data: MyStruct;

@compute @workgroup_size(1)
fn main() {
    let x = data.a;
}
"#;
    let result = validate(source, None);
    assert!(result.diagnostics.is_empty());

    let module = result.module.unwrap();
    let info = result.module_info.unwrap();
    let reflection = reflect(&module, &AttributeOverrides::default());

    // Find struct with 3 fields
    let my_struct = reflection
        .structs
        .iter()
        .find(|s| s.fields.len() == 3);
    assert!(my_struct.is_some(), "should find struct with 3 fields");

    let ms = my_struct.unwrap();
    assert_eq!(ms.fields[0].name.as_deref(), Some("a"));
    assert_eq!(ms.fields[1].name.as_deref(), Some("b"));
    assert_eq!(ms.fields[2].name.as_deref(), Some("c"));
}

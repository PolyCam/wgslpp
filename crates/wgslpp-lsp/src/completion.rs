//! Context-aware WGSL completions: keywords, builtins, user symbols, #include paths.

use lsp_types::{CompletionItem, CompletionItemKind, Position};

use crate::workspace::Workspace;

/// Compute completion items for a given position.
pub fn completions(workspace: &Workspace, uri: &str, position: Position) -> Vec<CompletionItem> {
    let source = match workspace.documents.get(uri) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let line = match source.lines().nth(position.line as usize) {
        Some(l) => l,
        None => return Vec::new(),
    };

    let col = position.character as usize;
    let prefix = if col <= line.len() {
        &line[..col]
    } else {
        line
    };
    let trimmed = prefix.trim();

    // Preprocessor directive completion
    if trimmed == "#" || trimmed.starts_with('#') {
        return preprocessor_completions(trimmed);
    }

    // #include path completion
    if trimmed.starts_with("#include ") {
        return include_path_completions(workspace, uri, trimmed);
    }

    // Get the partial word being typed
    let partial = extract_partial_word(prefix);

    let mut items = Vec::new();

    // User-defined symbols from naga module
    if let Some(snapshot) = workspace.get_snapshot(uri) {
        if let Some(module) = &snapshot.module {
            add_module_symbols(&mut items, module, &partial);
        }
    }

    // WGSL keywords
    add_keyword_completions(&mut items, &partial);

    // WGSL builtin functions
    add_builtin_completions(&mut items, &partial);

    items
}

fn extract_partial_word(prefix: &str) -> String {
    let bytes = prefix.as_bytes();
    let mut start = prefix.len();
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    prefix[start..].to_string()
}

fn preprocessor_completions(trimmed: &str) -> Vec<CompletionItem> {
    let directives = [
        ("include", "#include \"path\""),
        ("define", "#define NAME value"),
        ("undef", "#undef NAME"),
        ("ifdef", "#ifdef NAME"),
        ("ifndef", "#ifndef NAME"),
        ("if", "#if expression"),
        ("elif", "#elif expression"),
        ("else", "#else"),
        ("endif", "#endif"),
    ];

    let partial = trimmed.strip_prefix('#').unwrap_or("");

    directives
        .iter()
        .filter(|(name, _)| name.starts_with(partial))
        .map(|(name, detail)| CompletionItem {
            label: format!("#{}", name),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(detail.to_string()),
            ..Default::default()
        })
        .collect()
}

fn include_path_completions(
    workspace: &Workspace,
    _uri: &str,
    _trimmed: &str,
) -> Vec<CompletionItem> {
    // Offer package name completions for #include <
    // This is a simplified version — full implementation would scan directories
    let items = Vec::new();

    // TODO: scan package directories for path completions
    let _ = workspace;

    items
}

fn add_module_symbols(items: &mut Vec<CompletionItem>, module: &naga::Module, partial: &str) {
    // Functions
    for (_, func) in module.functions.iter() {
        if let Some(ref name) = func.name {
            if name.starts_with(partial) || partial.is_empty() {
                let params: Vec<String> = func
                    .arguments
                    .iter()
                    .map(|a| a.name.as_deref().unwrap_or("_").to_string())
                    .collect();
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!("fn({})", params.join(", "))),
                    ..Default::default()
                });
            }
        }
    }

    // Entry points
    for ep in &module.entry_points {
        if ep.name.starts_with(partial) || partial.is_empty() {
            items.push(CompletionItem {
                label: ep.name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(format!("@{:?} fn", ep.stage)),
                ..Default::default()
            });
        }
    }

    // Structs
    for (_, ty) in module.types.iter() {
        if let Some(ref name) = ty.name {
            if name.starts_with(partial) || partial.is_empty() {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::STRUCT),
                    detail: Some("struct".to_string()),
                    ..Default::default()
                });
            }
        }
    }

    // Global variables
    for (_, global) in module.global_variables.iter() {
        if let Some(ref name) = global.name {
            if name.starts_with(partial) || partial.is_empty() {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    ..Default::default()
                });
            }
        }
    }

    // Constants
    for (_, constant) in module.constants.iter() {
        if let Some(ref name) = constant.name {
            if name.starts_with(partial) || partial.is_empty() {
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    ..Default::default()
                });
            }
        }
    }
}

fn add_keyword_completions(items: &mut Vec<CompletionItem>, partial: &str) {
    let keywords = [
        "fn", "struct", "var", "let", "const", "return", "if", "else", "for", "while", "loop",
        "break", "continue", "switch", "case", "default", "discard", "override", "alias",
        "true", "false", "enable",
    ];

    for kw in &keywords {
        if kw.starts_with(partial) || partial.is_empty() {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                ..Default::default()
            });
        }
    }
}

fn add_builtin_completions(items: &mut Vec<CompletionItem>, partial: &str) {
    let builtins = [
        ("abs", "abs(e: T) -> T"),
        ("acos", "acos(e: f32) -> f32"),
        ("asin", "asin(e: f32) -> f32"),
        ("atan", "atan(e: f32) -> f32"),
        ("atan2", "atan2(y: f32, x: f32) -> f32"),
        ("ceil", "ceil(e: f32) -> f32"),
        ("clamp", "clamp(e: T, low: T, high: T) -> T"),
        ("cos", "cos(e: f32) -> f32"),
        ("cross", "cross(a: vec3<f32>, b: vec3<f32>) -> vec3<f32>"),
        ("degrees", "degrees(e: f32) -> f32"),
        ("distance", "distance(a: vecN<f32>, b: vecN<f32>) -> f32"),
        ("dot", "dot(a: vecN<T>, b: vecN<T>) -> T"),
        ("exp", "exp(e: f32) -> f32"),
        ("exp2", "exp2(e: f32) -> f32"),
        ("floor", "floor(e: f32) -> f32"),
        ("fma", "fma(a: f32, b: f32, c: f32) -> f32"),
        ("fract", "fract(e: f32) -> f32"),
        ("inverseSqrt", "inverseSqrt(e: f32) -> f32"),
        ("length", "length(e: vecN<f32>) -> f32"),
        ("log", "log(e: f32) -> f32"),
        ("log2", "log2(e: f32) -> f32"),
        ("max", "max(a: T, b: T) -> T"),
        ("min", "min(a: T, b: T) -> T"),
        ("mix", "mix(a: T, b: T, t: T) -> T"),
        ("normalize", "normalize(e: vecN<f32>) -> vecN<f32>"),
        ("pow", "pow(base: f32, exponent: f32) -> f32"),
        ("radians", "radians(e: f32) -> f32"),
        ("reflect", "reflect(e1: vecN<f32>, e2: vecN<f32>) -> vecN<f32>"),
        ("refract", "refract(e1: vecN<f32>, e2: vecN<f32>, eta: f32) -> vecN<f32>"),
        ("round", "round(e: f32) -> f32"),
        ("saturate", "saturate(e: f32) -> f32"),
        ("sign", "sign(e: T) -> T"),
        ("sin", "sin(e: f32) -> f32"),
        ("smoothstep", "smoothstep(low: f32, high: f32, x: f32) -> f32"),
        ("sqrt", "sqrt(e: f32) -> f32"),
        ("step", "step(edge: f32, x: f32) -> f32"),
        ("tan", "tan(e: f32) -> f32"),
        ("transpose", "transpose(m: matMxN<f32>) -> matNxM<f32>"),
        ("determinant", "determinant(m: matNxN<f32>) -> f32"),
        ("textureSample", "textureSample(t: texture_2d<f32>, s: sampler, coords: vec2<f32>) -> vec4<f32>"),
        ("textureLoad", "textureLoad(t: texture_2d<f32>, coords: vec2<i32>, level: i32) -> vec4<f32>"),
        ("textureStore", "textureStore(t: texture_storage_2d<...>, coords: vec2<i32>, value: vec4<f32>)"),
        ("textureDimensions", "textureDimensions(t: texture_2d<f32>) -> vec2<u32>"),
        ("select", "select(f: T, t: T, cond: bool) -> T"),
        ("arrayLength", "arrayLength(p: ptr<storage, array<T>>) -> u32"),
        ("atomicLoad", "atomicLoad(p: ptr<..., atomic<T>>) -> T"),
        ("atomicStore", "atomicStore(p: ptr<..., atomic<T>>, v: T)"),
        ("atomicAdd", "atomicAdd(p: ptr<..., atomic<T>>, v: T) -> T"),
        ("workgroupBarrier", "workgroupBarrier()"),
        ("storageBarrier", "storageBarrier()"),
    ];

    for (name, sig) in &builtins {
        if name.starts_with(partial) || partial.is_empty() {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some(sig.to_string()),
                ..Default::default()
            });
        }
    }

    // Builtin types
    let types = [
        "f32", "f16", "i32", "u32", "bool",
        "vec2", "vec3", "vec4",
        "mat2x2", "mat2x3", "mat2x4", "mat3x2", "mat3x3", "mat3x4",
        "mat4x2", "mat4x3", "mat4x4",
        "array", "atomic",
        "sampler", "sampler_comparison",
        "texture_1d", "texture_2d", "texture_2d_array", "texture_3d",
        "texture_cube", "texture_cube_array",
        "texture_multisampled_2d",
        "texture_depth_2d", "texture_depth_2d_array",
        "texture_depth_cube", "texture_depth_cube_array",
        "texture_depth_multisampled_2d",
        "texture_storage_1d", "texture_storage_2d", "texture_storage_2d_array",
        "texture_storage_3d",
    ];

    for ty in &types {
        if ty.starts_with(partial) || partial.is_empty() {
            items.push(CompletionItem {
                label: ty.to_string(),
                kind: Some(CompletionItemKind::TYPE_PARAMETER),
                ..Default::default()
            });
        }
    }
}

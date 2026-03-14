//! Frequency-based identifier renaming for naga modules.
//!
//! Counts usage frequency of each renameable identifier, then assigns
//! the shortest available names to the most frequently used identifiers.
//! Preserves entry point names and externally-visible binding names.

use std::collections::{HashMap, HashSet};

use naga::Module;

/// Rename identifiers in a naga module for minimal output size.
///
/// Preserves:
/// - Entry point function names (required by the GPU API)
/// - Global variables with `@group/@binding` decorations (host-visible)
///
/// Returns a mapping from original name to renamed name.
pub fn rename_identifiers(module: &mut Module) -> HashMap<String, String> {
    let mut name_map = HashMap::new();

    // Collect names that must be preserved
    let mut preserved: HashSet<String> = HashSet::new();
    for ep in &module.entry_points {
        preserved.insert(ep.name.clone());
    }
    for (_, global) in module.global_variables.iter() {
        if global.binding.is_some() {
            if let Some(ref name) = global.name {
                preserved.insert(name.clone());
            }
        }
    }

    // Count frequency of each renameable identifier
    let mut freq: HashMap<String, u32> = HashMap::new();

    // Functions
    for (_, func) in module.functions.iter() {
        if let Some(ref name) = func.name {
            if !preserved.contains(name) {
                *freq.entry(name.clone()).or_default() += count_references_in_function(func);
            }
        }
    }

    // Types (struct names)
    for (_, ty) in module.types.iter() {
        if let Some(ref name) = ty.name {
            if !preserved.contains(name) {
                // Struct names appear in declarations and type references
                *freq.entry(name.clone()).or_default() += 1;
            }
        }
        // Struct member names
        if let naga::TypeInner::Struct { ref members, .. } = ty.inner {
            for member in members {
                if let Some(ref name) = member.name {
                    if !preserved.contains(name) {
                        *freq.entry(name.clone()).or_default() += 1;
                    }
                }
            }
        }
    }

    // Global variables (non-binding)
    for (_, global) in module.global_variables.iter() {
        if global.binding.is_none() {
            if let Some(ref name) = global.name {
                if !preserved.contains(name) {
                    *freq.entry(name.clone()).or_default() += 1;
                }
            }
        }
    }

    // Constants
    for (_, constant) in module.constants.iter() {
        if let Some(ref name) = constant.name {
            if !preserved.contains(name) {
                *freq.entry(name.clone()).or_default() += 1;
            }
        }
    }

    // Sort by frequency (highest first)
    let mut sorted: Vec<(String, u32)> = freq.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Generate short names
    let mut gen = ShortNameGenerator::new();
    let mut used_names: HashSet<String> = preserved.clone();
    // Also reserve WGSL keywords
    for kw in WGSL_KEYWORDS {
        used_names.insert(kw.to_string());
    }

    for (original, _) in &sorted {
        let short = gen.next_unused(&used_names);
        used_names.insert(short.clone());
        name_map.insert(original.clone(), short);
    }

    // Apply renaming to the module
    apply_renames(module, &name_map, &preserved);

    name_map
}

/// Count how many times identifiers in a function are referenced
/// (rough heuristic — we count local variables and arguments too).
fn count_references_in_function(func: &naga::Function) -> u32 {
    let mut count = 1u32; // the function name itself
    // Count arguments
    count += func.arguments.len() as u32;
    // Count local variables
    count += func.local_variables.len() as u32;
    // Count named expressions
    count += func.named_expressions.len() as u32;
    count
}

fn apply_renames(module: &mut Module, renames: &HashMap<String, String>, preserved: &HashSet<String>) {
    // Rename functions
    for (_, func) in module.functions.iter_mut() {
        rename_opt(&mut func.name, renames, preserved);
        rename_function_internals(func, renames, preserved);
    }

    // Rename entry point function locals/args (but not the entry point name itself)
    for ep in &mut module.entry_points {
        rename_function_internals(&mut ep.function, renames, preserved);
    }

    // Rename types — UniqueArena is immutable, so we must rebuild it.
    // Collect all types, apply renames, then rebuild.
    let mut types_vec: Vec<(naga::Handle<naga::Type>, naga::Type)> = module.types.iter().map(|(h, t)| (h, t.clone())).collect();
    for (_, ty) in &mut types_vec {
        rename_opt(&mut ty.name, renames, preserved);
        if let naga::TypeInner::Struct { ref mut members, .. } = ty.inner {
            for member in members {
                rename_opt(&mut member.name, renames, preserved);
            }
        }
    }
    // Rebuild the UniqueArena. Since we only rename (not reorder/remove),
    // handles stay valid because UniqueArena::insert returns the same handle
    // for structurally identical types — and our renamed types maintain
    // their insertion order.
    let mut new_types = naga::UniqueArena::new();
    for (_, ty) in types_vec {
        new_types.insert(ty, Default::default());
    }
    module.types = new_types;

    // Rename global variables (non-binding)
    for (_, global) in module.global_variables.iter_mut() {
        if global.binding.is_none() {
            rename_opt(&mut global.name, renames, preserved);
        }
    }

    // Rename constants
    for (_, constant) in module.constants.iter_mut() {
        rename_opt(&mut constant.name, renames, preserved);
    }
}

fn rename_function_internals(func: &mut naga::Function, renames: &HashMap<String, String>, preserved: &HashSet<String>) {
    for arg in &mut func.arguments {
        rename_opt(&mut arg.name, renames, preserved);
    }
    for (_, local) in func.local_variables.iter_mut() {
        rename_opt(&mut local.name, renames, preserved);
    }
}

fn rename_opt(name: &mut Option<String>, renames: &HashMap<String, String>, preserved: &HashSet<String>) {
    if let Some(ref original) = name {
        if preserved.contains(original) {
            return;
        }
        if let Some(new_name) = renames.get(original) {
            *name = Some(new_name.clone());
        }
    }
}

/// Generates short identifier names: a, b, ..., z, A, ..., Z, aa, ab, ...
struct ShortNameGenerator {
    counter: usize,
}

impl ShortNameGenerator {
    fn new() -> Self {
        Self { counter: 0 }
    }

    fn nth(&self, n: usize) -> String {
        // First 52: a-z, A-Z. Then aa, ab, ..., az, aA, ..., aZ, ba, ...
        let chars: Vec<char> = ('a'..='z').chain('A'..='Z').collect();
        let base = chars.len(); // 52

        if n < base {
            return chars[n].to_string();
        }

        let n = n - base;
        let mut result = String::new();
        // First character: from a-z,A-Z but offset
        let first_idx = n / base;
        let rest_idx = n % base;

        if first_idx < base {
            result.push(chars[first_idx]);
            result.push(chars[rest_idx]);
        } else {
            // For very long names, fall back to _N pattern
            result = format!("_{}", n);
        }

        result
    }

    fn next_unused(&mut self, used: &HashSet<String>) -> String {
        loop {
            let name = self.nth(self.counter);
            self.counter += 1;
            if !used.contains(&name) {
                return name;
            }
        }
    }
}

/// WGSL reserved keywords that cannot be used as identifiers.
const WGSL_KEYWORDS: &[&str] = &[
    "alias", "break", "case", "const", "const_assert", "continue", "continuing",
    "default", "diagnostic", "discard", "else", "enable", "false", "fn", "for",
    "if", "let", "loop", "override", "return", "struct", "switch", "true", "var",
    "while",
    // Reserved words
    "NULL", "Self", "abstract", "active", "alignas", "alignof", "as", "asm",
    "asm_fragment", "async", "attribute", "auto", "await", "become", "binding_array",
    "cast", "catch", "class", "co_await", "co_return", "co_yield", "coherent",
    "column_major", "common", "compile", "compile_fragment", "concept", "const_cast",
    "consteval", "constexpr", "constinit", "crate", "debugger", "decltype", "delete",
    "demote", "demote_to_helper", "do", "dynamic_cast", "enum", "explicit", "export",
    "extends", "extern", "external", "fallthrough", "filter", "final", "finally",
    "friend", "from", "fxgroup", "get", "goto", "groupshared", "highp", "impl",
    "implements", "import", "in", "inline", "instanceof", "interface", "layout",
    "lowp", "macro", "macro_rules", "match", "mediump", "meta", "mod", "module",
    "move", "mut", "mutable", "namespace", "new", "nil", "noexcept", "noinline",
    "nointerpolation", "noperspective", "null", "nullptr", "of", "operator", "package",
    "packoffset", "partition", "pass", "patch", "pixelfragment", "precise", "precision",
    "premerge", "priv", "protected", "pub", "public", "readonly", "ref", "regardless",
    "register", "reinterpret_cast", "require", "resource", "restrict", "self",
    "set", "shared", "sizeof", "smooth", "snorm", "static", "static_assert",
    "static_cast", "std", "subroutine", "super", "target", "template", "this",
    "thread_local", "throw", "trait", "try", "type", "typedef", "typeid", "typename",
    "typeof", "union", "unless", "unorm", "unsafe", "unsized", "use", "using",
    "varying", "virtual", "volatile", "wgsl", "where", "with", "writeonly", "yield",
    // Builtin value names
    "vec2", "vec3", "vec4", "mat2x2", "mat2x3", "mat2x4", "mat3x2", "mat3x3",
    "mat3x4", "mat4x2", "mat4x3", "mat4x4", "f32", "f16", "i32", "u32", "bool",
    "array", "ptr", "sampler", "sampler_comparison",
    "texture_1d", "texture_2d", "texture_2d_array", "texture_3d", "texture_cube",
    "texture_cube_array", "texture_multisampled_2d", "texture_storage_1d",
    "texture_storage_2d", "texture_storage_2d_array", "texture_storage_3d",
    "texture_depth_2d", "texture_depth_2d_array", "texture_depth_cube",
    "texture_depth_cube_array", "texture_depth_multisampled_2d",
    "texture_external",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate;

    #[test]
    fn test_short_name_generation() {
        let gen = ShortNameGenerator::new();
        assert_eq!(gen.nth(0), "a");
        assert_eq!(gen.nth(25), "z");
        assert_eq!(gen.nth(26), "A");
        assert_eq!(gen.nth(51), "Z");
        assert_eq!(gen.nth(52), "aa");
        assert_eq!(gen.nth(53), "ab");
    }

    #[test]
    fn test_rename_preserves_entry_points() {
        let source = r#"
struct MyStruct {
    value: f32,
}

fn helper(x: f32) -> f32 {
    return x * 2.0;
}

@fragment
fn main() -> @location(0) vec4<f32> {
    let result = helper(1.0);
    return vec4<f32>(result, 0.0, 0.0, 1.0);
}
"#;
        let result = validate(source, None);
        let mut module = result.module.unwrap();

        let renames = rename_identifiers(&mut module);

        // Entry point "main" should NOT be renamed
        assert!(!renames.contains_key("main"));

        // "helper" and "MyStruct" should be renamed to short names
        assert!(renames.contains_key("helper"));
        assert!(renames.contains_key("MyStruct"));

        // Verify the entry point name is preserved in the module
        assert_eq!(module.entry_points[0].name, "main");
    }

    #[test]
    fn test_rename_preserves_bindings() {
        let source = r#"
@group(0) @binding(0)
var<uniform> myUniform: vec4<f32>;

@fragment
fn main() -> @location(0) vec4<f32> {
    return myUniform;
}
"#;
        let result = validate(source, None);
        let mut module = result.module.unwrap();

        let renames = rename_identifiers(&mut module);

        // Binding name should be preserved
        assert!(!renames.contains_key("myUniform"));
    }

    #[test]
    fn test_rename_does_not_use_keywords() {
        let used: HashSet<String> = HashSet::new();
        let mut gen = ShortNameGenerator::new();

        // WGSL keywords set
        let mut reserved = used;
        for kw in WGSL_KEYWORDS {
            reserved.insert(kw.to_string());
        }

        // Generate several names and ensure none are keywords
        for _ in 0..100 {
            let name = gen.next_unused(&reserved);
            assert!(
                !WGSL_KEYWORDS.contains(&name.as_str()),
                "generated keyword: {}",
                name
            );
            reserved.insert(name);
        }
    }
}

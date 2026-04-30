//! Per-binding attribute overrides surfaced via WGSL doc comments.
//!
//! Markers are written as triple-slash doc comments on a `var` declaration,
//! e.g.
//! ```wgsl
//! /// @unfilterable
//! @group(0) @binding(0) var depth_tex: texture_2d<f32>;
//! ```
//!
//! Naga's WGSL frontend collects these into `Module.doc_comments` when the
//! parser is constructed with `parse_doc_comments: true`. We walk that map
//! instead of re-scanning the source — the parser has already done the hard
//! work of attaching each comment to the right binding handle.
//!
//! Recognised markers (the entire trimmed comment body must equal one of
//! these, case-sensitive):
//!   - `@unfilterable`   — texture binding
//!   - `@nonfiltering`   — sampler binding
//!   - `@dynamic_offset` — uniform / storage buffer binding

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Per-binding overrides surfaced via doc-comment markers. Defaults to empty
/// (no overrides) so consumers that don't care can pass
/// `&AttributeOverrides::default()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttributeOverrides {
    /// Var names marked `/// @unfilterable` (textures).
    pub unfilterable: HashSet<String>,
    /// Var names marked `/// @nonfiltering` (samplers).
    pub nonfiltering: HashSet<String>,
    /// Var names marked `/// @dynamic_offset` (uniform/storage buffers).
    pub dynamic_offset: HashSet<String>,
}

/// Pull marker overrides off a parsed naga module's doc-comment map.
///
/// `module.doc_comments` is `None` when the parser was run without doc-comment
/// support, or when no doc comments appeared in the source — in either case
/// we return an empty `AttributeOverrides`.
pub fn extract_attributes(module: &naga::Module) -> AttributeOverrides {
    let mut out = AttributeOverrides::default();

    let Some(doc_comments) = module.doc_comments.as_deref() else {
        return out;
    };

    for (handle, comments) in &doc_comments.global_variables {
        let Some(name) = module.global_variables[*handle].name.as_deref() else {
            continue;
        };
        for comment in comments {
            match classify(comment) {
                Some(Marker::Unfilterable) => {
                    out.unfilterable.insert(name.to_string());
                }
                Some(Marker::Nonfiltering) => {
                    out.nonfiltering.insert(name.to_string());
                }
                Some(Marker::DynamicOffset) => {
                    out.dynamic_offset.insert(name.to_string());
                }
                None => {}
            }
        }
    }

    out
}

#[derive(Copy, Clone)]
enum Marker {
    Unfilterable,
    Nonfiltering,
    DynamicOffset,
}

/// Match a single doc-comment string against the marker vocabulary.
///
/// Naga preserves the leading `///` in the stored string, so we strip it
/// (and any surrounding whitespace) before comparing. The body must equal
/// exactly one of the marker tokens — any extra prose means we leave it
/// alone, so descriptive `///` comments and markers don't collide.
fn classify(comment: &str) -> Option<Marker> {
    let body = comment
        .trim_start()
        .strip_prefix("///")
        .unwrap_or(comment)
        .trim();
    match body {
        "@unfilterable" => Some(Marker::Unfilterable),
        "@nonfiltering" => Some(Marker::Nonfiltering),
        "@dynamic_offset" => Some(Marker::DynamicOffset),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate;

    fn parse(source: &str) -> naga::Module {
        let result = validate(source, None);
        result
            .module
            .unwrap_or_else(|| panic!("parse failed: {:?}", result.diagnostics))
    }

    #[test]
    fn unfilterable_on_texture() {
        let module = parse(
            r#"
            /// @unfilterable
            @group(2) @binding(2) var depth_image: texture_2d<f32>;
            @fragment fn fs() -> @location(0) vec4<f32> {
                let v = textureLoad(depth_image, vec2<u32>(0, 0), 0);
                return v;
            }
            "#,
        );
        let attrs = extract_attributes(&module);
        assert!(attrs.unfilterable.contains("depth_image"));
        assert!(attrs.nonfiltering.is_empty());
    }

    #[test]
    fn nonfiltering_on_sampler() {
        let module = parse(
            r#"
            @group(2) @binding(2) var depth_image: texture_2d<f32>;
            /// @nonfiltering
            @group(2) @binding(3) var depth_sampler: sampler;
            @fragment fn fs() -> @location(0) vec4<f32> {
                return textureSample(depth_image, depth_sampler, vec2<f32>(0.0));
            }
            "#,
        );
        let attrs = extract_attributes(&module);
        assert!(attrs.nonfiltering.contains("depth_sampler"));
        assert!(attrs.unfilterable.is_empty());
    }

    #[test]
    fn dynamic_offset_on_uniform_buffer() {
        let module = parse(
            r#"
            struct PerRenderableData { x: f32 }
            /// @dynamic_offset
            @group(1) @binding(0) var<uniform> renderable: PerRenderableData;
            @fragment fn fs() -> @location(0) vec4<f32> {
                return vec4<f32>(renderable.x, 0.0, 0.0, 1.0);
            }
            "#,
        );
        let attrs = extract_attributes(&module);
        assert!(attrs.dynamic_offset.contains("renderable"));
    }

    #[test]
    fn ignores_unrelated_doc_comments() {
        let module = parse(
            r#"
            /// just a regular doc comment about this binding
            @group(0) @binding(0) var tex: texture_2d<f32>;
            @fragment fn fs() -> @location(0) vec4<f32> {
                return textureLoad(tex, vec2<u32>(0, 0), 0);
            }
            "#,
        );
        let attrs = extract_attributes(&module);
        assert!(attrs.unfilterable.is_empty());
        assert!(attrs.nonfiltering.is_empty());
        assert!(attrs.dynamic_offset.is_empty());
    }

    #[test]
    fn double_slash_comments_are_ignored() {
        // Plain `//` comments are not doc comments — naga drops them, so they
        // must not be picked up as markers.
        let module = parse(
            r#"
            // @unfilterable
            @group(2) @binding(2) var depth_image: texture_2d<f32>;
            @fragment fn fs() -> @location(0) vec4<f32> {
                return textureLoad(depth_image, vec2<u32>(0, 0), 0);
            }
            "#,
        );
        let attrs = extract_attributes(&module);
        assert!(attrs.unfilterable.is_empty());
    }

    #[test]
    fn empty_module_yields_empty_overrides() {
        let module = parse("@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(0.0); }");
        let attrs = extract_attributes(&module);
        assert!(attrs.unfilterable.is_empty());
        assert!(attrs.nonfiltering.is_empty());
        assert!(attrs.dynamic_offset.is_empty());
    }
}

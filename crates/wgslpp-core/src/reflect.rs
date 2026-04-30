use crate::attributes::AttributeOverrides;
use naga::common::wgsl::ToWgsl;
use serde::{Deserialize, Serialize};

/// Reflection data extracted from a validated naga module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionData {
    pub bindings: Vec<BindingInfo>,
    pub structs: Vec<StructInfo>,
    pub entry_points: Vec<EntryPointInfo>,
}

fn is_false(b: &bool) -> bool {
    !b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingInfo {
    pub group: u32,
    pub binding: u32,
    pub name: Option<String>,
    /// Binding kind + access. One of:
    /// `uniform` / `storage_read` / `storage_read_write` /
    /// `sampler` / `sampler_comparison` / `texture`.
    /// Drives BindGroupLayoutEntry buffer/sampler dispatch.
    #[serde(rename = "type")]
    pub binding_type: String,
    /// The WGSL type that follows the colon in the var declaration:
    ///   - buffers:  the struct name (e.g. `FrameUniforms`)
    ///   - textures: the full texture type (e.g. `texture_2d<f32>`)
    ///   - samplers: `sampler` or `sampler_comparison`
    /// For textures this is what `BindGroupLayoutEntry.texture` is dispatched
    /// from; for buffers it's the key for looking up the underlying struct.
    pub wgsl_type: String,
    /// True when the texture binding is multi-sampled (intrinsically
    /// unfilterable per WebGPU spec) or marked `// @unfilterable` in the
    /// WGSL source. Meaningless for non-texture bindings.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unfilterable: bool,
    /// True when the sampler binding is marked `// @nonfiltering` in the
    /// WGSL source. Meaningless for non-sampler bindings.
    #[serde(default, skip_serializing_if = "is_false")]
    pub nonfiltering: bool,
    /// True when the uniform/storage buffer binding is marked
    /// `// @dynamic_offset` in the WGSL source. Meaningless for non-buffer
    /// bindings.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dynamic_offset: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructInfo {
    pub name: Option<String>,
    pub size: u32,
    pub alignment: u32,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub field_type: String,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPointInfo {
    pub name: String,
    pub stage: String,
    pub workgroup_size: Option<[u32; 3]>,
}

/// Extract reflection data from a parsed naga module. `attributes` carries
/// per-binding overrides surfaced by the WGSL marker-comment scan
/// (`// @unfilterable`, `// @nonfiltering`); pass
/// `&AttributeOverrides::default()` if you don't care.
pub fn reflect(module: &naga::Module, attributes: &AttributeOverrides) -> ReflectionData {
    let bindings = extract_bindings(module, attributes);
    let structs = extract_structs(module);
    let entry_points = extract_entry_points(module);

    ReflectionData {
        bindings,
        structs,
        entry_points,
    }
}

fn extract_bindings(
    module: &naga::Module,
    attributes: &AttributeOverrides,
) -> Vec<BindingInfo> {
    let mut bindings = Vec::new();

    for (_, global) in module.global_variables.iter() {
        if let Some(ref binding) = global.binding {
            let ty = &module.types[global.ty];
            let ty_inner = &ty.inner;

            // Kind + access — drives buffer/sampler dispatch in layout-entry
            // generation. Texture variants share a single `texture` kind; the
            // texture-specific shape lives in `wgsl_type`.
            let binding_type = match (global.space, ty_inner) {
                (naga::AddressSpace::Uniform, _) => "uniform",
                (naga::AddressSpace::Storage { access }, _) => {
                    if access.contains(naga::StorageAccess::STORE) {
                        "storage_read_write"
                    } else {
                        "storage_read"
                    }
                }
                (naga::AddressSpace::Handle, naga::TypeInner::Sampler { comparison: true }) => {
                    "sampler_comparison"
                }
                (naga::AddressSpace::Handle, naga::TypeInner::Sampler { comparison: false }) => {
                    "sampler"
                }
                (naga::AddressSpace::Handle, naga::TypeInner::Image { .. }) => "texture",
                _ => "other",
            };

            // The WGSL type that follows `:` in the var declaration. For
            // buffers this is the struct name (e.g. `FrameUniforms`); for
            // textures the full type (e.g. `texture_2d<f32>`); for samplers
            // either `sampler` or `sampler_comparison`.
            let wgsl_type = match (global.space, ty_inner) {
                (naga::AddressSpace::Uniform, _) | (naga::AddressSpace::Storage { .. }, _) => {
                    ty.name.clone().unwrap_or_default()
                }
                (naga::AddressSpace::Handle, naga::TypeInner::Sampler { comparison: true }) => {
                    "sampler_comparison".to_string()
                }
                (naga::AddressSpace::Handle, naga::TypeInner::Sampler { comparison: false }) => {
                    "sampler".to_string()
                }
                (
                    naga::AddressSpace::Handle,
                    naga::TypeInner::Image {
                        dim,
                        arrayed,
                        class,
                    },
                ) => wgsl_image_type(*dim, *arrayed, class),
                _ => String::new(),
            };

            // Multi-sampled textures are intrinsically unfilterable per
            // WebGPU spec; the `// @unfilterable` marker is the explicit
            // override path for non-multi-sampled textures bound with
            // formats that aren't filterable in the core spec (e.g. r32f
            // without the float32-filterable feature).
            let is_multisampled = matches!(
                ty_inner,
                naga::TypeInner::Image {
                    class: naga::ImageClass::Sampled { multi: true, .. },
                    ..
                }
            );
            let unfilterable = match (binding_type, &global.name) {
                ("texture", _) if is_multisampled => true,
                ("texture", Some(name)) if attributes.unfilterable.contains(name) => true,
                _ => false,
            };
            let nonfiltering = match (binding_type, &global.name) {
                ("sampler", Some(name)) if attributes.nonfiltering.contains(name) => true,
                _ => false,
            };
            let dynamic_offset = match (binding_type, &global.name) {
                ("uniform" | "storage_read" | "storage_read_write", Some(name))
                    if attributes.dynamic_offset.contains(name) =>
                {
                    true
                }
                _ => false,
            };

            bindings.push(BindingInfo {
                group: binding.group,
                binding: binding.binding,
                name: global.name.clone(),
                binding_type: binding_type.to_string(),
                wgsl_type,
                unfilterable,
                nonfiltering,
                dynamic_offset,
            });
        }
    }

    bindings.sort_by_key(|b| (b.group, b.binding));
    bindings
}

fn wgsl_image_type(
    dim: naga::ImageDimension,
    arrayed: bool,
    class: &naga::ImageClass,
) -> String {
    let dim_str = match dim {
        naga::ImageDimension::D1 => "1d",
        naga::ImageDimension::D2 => "2d",
        naga::ImageDimension::D3 => "3d",
        naga::ImageDimension::Cube => "cube",
    };
    let array_suffix = if arrayed { "_array" } else { "" };

    match class {
        naga::ImageClass::Sampled { kind, multi } => {
            let scalar = match kind {
                naga::ScalarKind::Float => "f32",
                naga::ScalarKind::Sint => "i32",
                naga::ScalarKind::Uint => "u32",
                _ => "f32",
            };
            if *multi {
                format!("texture_multisampled_{dim_str}{array_suffix}<{scalar}>")
            } else {
                format!("texture_{dim_str}{array_suffix}<{scalar}>")
            }
        }
        naga::ImageClass::Depth { multi } => {
            if *multi {
                format!("texture_depth_multisampled_{dim_str}{array_suffix}")
            } else {
                format!("texture_depth_{dim_str}{array_suffix}")
            }
        }
        naga::ImageClass::External => {
            format!("texture_external")
        }
        naga::ImageClass::Storage { format, access } => {
            let format_str = format.to_wgsl();
            let access_str = if access.contains(naga::StorageAccess::STORE)
                && access.contains(naga::StorageAccess::LOAD)
            {
                "read_write"
            } else if access.contains(naga::StorageAccess::STORE) {
                "write"
            } else {
                "read"
            };
            format!("texture_storage_{dim_str}{array_suffix}<{format_str}, {access_str}>")
        }
    }
}

fn extract_structs(module: &naga::Module) -> Vec<StructInfo> {
    let mut structs = Vec::new();

    for (_handle, ty) in module.types.iter() {
        if let naga::TypeInner::Struct { members, span } = &ty.inner {
            let fields: Vec<FieldInfo> = members
                .iter()
                .map(|member| {
                    let field_type = type_name(&module.types, &member.ty);
                    let size = module.types[member.ty].inner.size(module.to_ctx());
                    FieldInfo {
                        name: member.name.clone(),
                        field_type,
                        offset: member.offset,
                        size,
                    }
                })
                .collect();

            structs.push(StructInfo {
                name: ty.name.clone(),
                size: *span,
                alignment: 0, // naga 28 TypeFlags doesn't expose alignment directly
                fields,
            });
        }
    }

    structs
}

fn extract_entry_points(module: &naga::Module) -> Vec<EntryPointInfo> {
    module
        .entry_points
        .iter()
        .map(|ep| {
            let stage = match ep.stage {
                naga::ShaderStage::Vertex => "vertex",
                naga::ShaderStage::Fragment => "fragment",
                naga::ShaderStage::Compute => "compute",
                naga::ShaderStage::Task => "task",
                naga::ShaderStage::Mesh => "mesh",
            };
            let workgroup_size = if ep.stage == naga::ShaderStage::Compute {
                Some(ep.workgroup_size)
            } else {
                None
            };
            EntryPointInfo {
                name: ep.name.clone(),
                stage: stage.to_string(),
                workgroup_size,
            }
        })
        .collect()
}

/// Get a human-readable type name for reflection output.
fn type_name(types: &naga::UniqueArena<naga::Type>, handle: &naga::Handle<naga::Type>) -> String {
    let ty = &types[*handle];
    if let Some(ref name) = ty.name {
        return name.clone();
    }
    match &ty.inner {
        naga::TypeInner::Scalar(scalar) => scalar_name(scalar),
        naga::TypeInner::Vector { size, scalar } => {
            format!("vec{}<{}>", *size as u8, scalar_name(scalar))
        }
        naga::TypeInner::Matrix {
            columns,
            rows,
            scalar,
        } => {
            format!(
                "mat{}x{}<{}>",
                *columns as u8,
                *rows as u8,
                scalar_name(scalar)
            )
        }
        naga::TypeInner::Array { base, size, .. } => {
            let base_name = type_name(types, base);
            match size {
                naga::ArraySize::Constant(len) => format!("array<{}, {}>", base_name, len),
                naga::ArraySize::Dynamic => format!("array<{}>", base_name),
                naga::ArraySize::Pending(_) => format!("array<{}, ?>", base_name),
            }
        }
        naga::TypeInner::Struct { .. } => "<struct>".to_string(),
        naga::TypeInner::Image { dim, class, .. } => {
            format!("texture_{:?}_{:?}", dim, class)
        }
        naga::TypeInner::Sampler { comparison } => {
            if *comparison {
                "sampler_comparison".to_string()
            } else {
                "sampler".to_string()
            }
        }
        naga::TypeInner::Pointer { base, space } => {
            format!("ptr<{:?}, {}>", space, type_name(types, base))
        }
        _ => format!("{:?}", ty.inner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate;

    fn reflect_source(source: &str) -> ReflectionData {
        let result = validate(source, None);
        let module = result.module.unwrap();
        reflect(&module, &AttributeOverrides::default())
    }

    #[test]
    fn test_uniform_buffer() {
        let data = reflect_source(
            r#"
            struct MyUniforms { x: f32 }
            @group(0) @binding(0) var<uniform> u: MyUniforms;
            @fragment fn main() -> @location(0) vec4<f32> {
                return vec4<f32>(u.x, 0.0, 0.0, 1.0);
            }
            "#,
        );
        assert_eq!(data.bindings.len(), 1);
        assert_eq!(data.bindings[0].binding_type, "uniform");
        assert_eq!(data.bindings[0].wgsl_type, "MyUniforms");
    }

    #[test]
    fn test_storage_read() {
        let data = reflect_source(
            r#"
            struct Data { values: array<f32> }
            @group(0) @binding(0) var<storage, read> buf: Data;
            @fragment fn main() -> @location(0) vec4<f32> {
                return vec4<f32>(buf.values[0], 0.0, 0.0, 1.0);
            }
            "#,
        );
        assert_eq!(data.bindings[0].binding_type, "storage_read");
        assert_eq!(data.bindings[0].wgsl_type, "Data");
    }

    #[test]
    fn test_storage_read_write() {
        let data = reflect_source(
            r#"
            struct Data { values: array<f32> }
            @group(0) @binding(0) var<storage, read_write> buf: Data;
            @compute @workgroup_size(1) fn main() {
                buf.values[0] = 1.0;
            }
            "#,
        );
        assert_eq!(data.bindings[0].binding_type, "storage_read_write");
        assert_eq!(data.bindings[0].wgsl_type, "Data");
    }

    #[test]
    fn test_texture_2d_f32() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_2d<f32>;
            @group(0) @binding(1) var samp: sampler;
            @fragment fn main() -> @location(0) vec4<f32> {
                return textureSample(tex, samp, vec2<f32>(0.0, 0.0));
            }
            "#,
        );
        let tex = data.bindings.iter().find(|b| b.binding == 0).unwrap();
        let samp = data.bindings.iter().find(|b| b.binding == 1).unwrap();
        assert_eq!(tex.binding_type, "texture");
        assert_eq!(tex.wgsl_type, "texture_2d<f32>");
        assert_eq!(samp.binding_type, "sampler");
        assert_eq!(samp.wgsl_type, "sampler");
    }

    #[test]
    fn test_texture_cube() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_cube<f32>;
            @group(0) @binding(1) var samp: sampler;
            @fragment fn main() -> @location(0) vec4<f32> {
                return textureSample(tex, samp, vec3<f32>(0.0, 0.0, 1.0));
            }
            "#,
        );
        let tex = data.bindings.iter().find(|b| b.binding == 0).unwrap();
        assert_eq!(tex.wgsl_type, "texture_cube<f32>");
    }

    #[test]
    fn test_texture_depth_2d() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_depth_2d;
            @group(0) @binding(1) var samp: sampler_comparison;
            @fragment fn main() -> @location(0) vec4<f32> {
                let d = textureSampleCompare(tex, samp, vec2<f32>(0.0, 0.0), 0.5);
                return vec4<f32>(d, 0.0, 0.0, 1.0);
            }
            "#,
        );
        let tex = data.bindings.iter().find(|b| b.binding == 0).unwrap();
        let samp = data.bindings.iter().find(|b| b.binding == 1).unwrap();
        assert_eq!(tex.wgsl_type, "texture_depth_2d");
        assert_eq!(samp.wgsl_type, "sampler_comparison");
    }

    #[test]
    fn test_texture_depth_2d_array() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_depth_2d_array;
            @group(0) @binding(1) var samp: sampler_comparison;
            @fragment fn main() -> @location(0) vec4<f32> {
                let d = textureSampleCompare(tex, samp, vec2<f32>(0.0, 0.0), 0, 0.5);
                return vec4<f32>(d, 0.0, 0.0, 1.0);
            }
            "#,
        );
        let tex = data.bindings.iter().find(|b| b.binding == 0).unwrap();
        assert_eq!(tex.wgsl_type, "texture_depth_2d_array");
    }

    #[test]
    fn test_texture_2d_i32() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_2d<i32>;
            @fragment fn main() -> @location(0) vec4<f32> {
                let v = textureLoad(tex, vec2<u32>(0, 0), 0);
                return vec4<f32>(f32(v.x), 0.0, 0.0, 1.0);
            }
            "#,
        );
        assert_eq!(data.bindings[0].wgsl_type, "texture_2d<i32>");
    }

    #[test]
    fn test_texture_3d() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_3d<f32>;
            @group(0) @binding(1) var samp: sampler;
            @fragment fn main() -> @location(0) vec4<f32> {
                return textureSample(tex, samp, vec3<f32>(0.0, 0.0, 0.0));
            }
            "#,
        );
        assert_eq!(data.bindings[0].wgsl_type, "texture_3d<f32>");
    }

    #[test]
    fn test_texture_storage_2d() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_storage_2d<rgba8unorm, write>;
            @compute @workgroup_size(1) fn main() {
                textureStore(tex, vec2<u32>(0, 0), vec4<f32>(1.0, 0.0, 0.0, 1.0));
            }
            "#,
        );
        assert_eq!(data.bindings[0].wgsl_type, "texture_storage_2d<rgba8unorm, write>");
    }

    #[test]
    fn test_texture_multisampled_2d() {
        let data = reflect_source(
            r#"
            @group(0) @binding(0) var tex: texture_multisampled_2d<f32>;
            @fragment fn main() -> @location(0) vec4<f32> {
                return textureLoad(tex, vec2<u32>(0, 0), 0);
            }
            "#,
        );
        assert_eq!(data.bindings[0].wgsl_type, "texture_multisampled_2d<f32>");
    }
}

fn scalar_name(scalar: &naga::Scalar) -> String {
    match (scalar.kind, scalar.width) {
        (naga::ScalarKind::Float, 4) => "f32".to_string(),
        (naga::ScalarKind::Float, 8) => "f64".to_string(),
        (naga::ScalarKind::Float, 2) => "f16".to_string(),
        (naga::ScalarKind::Sint, 4) => "i32".to_string(),
        (naga::ScalarKind::Uint, 4) => "u32".to_string(),
        (naga::ScalarKind::Bool, _) => "bool".to_string(),
        (naga::ScalarKind::AbstractInt, _) => "abstract_int".to_string(),
        (naga::ScalarKind::AbstractFloat, _) => "abstract_float".to_string(),
        _ => format!("{:?}_{}", scalar.kind, scalar.width),
    }
}

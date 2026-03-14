use naga::valid::ModuleInfo;
use serde::Serialize;

/// Reflection data extracted from a validated naga module.
#[derive(Debug, Clone, Serialize)]
pub struct ReflectionData {
    pub bindings: Vec<BindingInfo>,
    pub structs: Vec<StructInfo>,
    pub entry_points: Vec<EntryPointInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BindingInfo {
    pub group: u32,
    pub binding: u32,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub binding_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructInfo {
    pub name: Option<String>,
    pub size: u32,
    pub alignment: u32,
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldInfo {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub field_type: String,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryPointInfo {
    pub name: String,
    pub stage: String,
    pub workgroup_size: Option<[u32; 3]>,
}

/// Extract reflection data from a validated naga module.
pub fn reflect(module: &naga::Module, module_info: &ModuleInfo) -> ReflectionData {
    let bindings = extract_bindings(module);
    let structs = extract_structs(module, module_info);
    let entry_points = extract_entry_points(module);

    ReflectionData {
        bindings,
        structs,
        entry_points,
    }
}

fn extract_bindings(module: &naga::Module) -> Vec<BindingInfo> {
    let mut bindings = Vec::new();

    for (_, global) in module.global_variables.iter() {
        if let Some(ref binding) = global.binding {
            let binding_type = match global.space {
                naga::AddressSpace::Uniform => "uniform",
                naga::AddressSpace::Storage { access } => {
                    if access.contains(naga::StorageAccess::STORE) {
                        "storage_rw"
                    } else {
                        "storage"
                    }
                }
                naga::AddressSpace::Handle => {
                    // Determine if texture or sampler from the type
                    match module.types[global.ty].inner {
                        naga::TypeInner::Sampler { .. } => "sampler",
                        naga::TypeInner::Image { .. } => "texture",
                        _ => "handle",
                    }
                }
                _ => "other",
            };

            bindings.push(BindingInfo {
                group: binding.group,
                binding: binding.binding,
                name: global.name.clone(),
                binding_type: binding_type.to_string(),
            });
        }
    }

    bindings.sort_by_key(|b| (b.group, b.binding));
    bindings
}

fn extract_structs(module: &naga::Module, _module_info: &ModuleInfo) -> Vec<StructInfo> {
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

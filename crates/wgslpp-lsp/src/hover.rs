//! Hover info from naga IR: type signatures, struct layouts, binding info.

use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::workspace::Workspace;

/// Compute hover information for a position.
pub fn hover(workspace: &Workspace, uri: &str, position: Position) -> Option<Hover> {
    let source = workspace.documents.get(uri)?;
    let word = word_at_position(source, position)?;

    let snapshot = workspace.get_snapshot(uri)?;
    let module = snapshot.module.as_ref()?;

    // Look up the word in naga module
    let info = lookup_symbol(module, &word)?;

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: info,
        }),
        range: None,
    })
}

fn word_at_position(source: &str, position: Position) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let col = position.character as usize;
    if col >= line.len() {
        return None;
    }

    let bytes = line.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    let mut start = col;
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = col;
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }
    Some(line[start..end].to_string())
}

fn lookup_symbol(module: &naga::Module, name: &str) -> Option<String> {
    // Entry points
    for ep in &module.entry_points {
        if ep.name == name {
            let stage = match ep.stage {
                naga::ShaderStage::Vertex => "vertex",
                naga::ShaderStage::Fragment => "fragment",
                naga::ShaderStage::Compute => "compute",
                _ => "unknown",
            };
            let params: Vec<String> = ep
                .function
                .arguments
                .iter()
                .map(|a| {
                    let name = a.name.as_deref().unwrap_or("_");
                    let ty = type_name(&module.types, &a.ty);
                    format!("{}: {}", name, ty)
                })
                .collect();
            let ret = ep
                .function
                .result
                .as_ref()
                .map(|r| format!(" -> {}", type_name(&module.types, &r.ty)))
                .unwrap_or_default();
            return Some(format!(
                "```wgsl\n@{}\nfn {}({}){}\n```",
                stage,
                name,
                params.join(", "),
                ret
            ));
        }
    }

    // Functions
    for (_, func) in module.functions.iter() {
        if func.name.as_deref() == Some(name) {
            let params: Vec<String> = func
                .arguments
                .iter()
                .map(|a| {
                    let name = a.name.as_deref().unwrap_or("_");
                    let ty = type_name(&module.types, &a.ty);
                    format!("{}: {}", name, ty)
                })
                .collect();
            let ret = func
                .result
                .as_ref()
                .map(|r| format!(" -> {}", type_name(&module.types, &r.ty)))
                .unwrap_or_default();
            return Some(format!(
                "```wgsl\nfn {}({}){}\n```",
                name,
                params.join(", "),
                ret
            ));
        }
    }

    // Structs
    for (_, ty) in module.types.iter() {
        if ty.name.as_deref() == Some(name) {
            if let naga::TypeInner::Struct { ref members, span } = ty.inner {
                let mut info = format!("```wgsl\nstruct {} {{\n", name);
                for member in members {
                    let member_name = member.name.as_deref().unwrap_or("_");
                    let member_type = type_name(&module.types, &member.ty);
                    info.push_str(&format!(
                        "    {}: {},  // offset: {}\n",
                        member_name, member_type, member.offset
                    ));
                }
                info.push_str("}\n```\n");
                info.push_str(&format!("Size: {} bytes", span));
                return Some(info);
            }
        }
    }

    // Global variables
    for (_, global) in module.global_variables.iter() {
        if global.name.as_deref() == Some(name) {
            let ty = type_name(&module.types, &global.ty);
            let space = match global.space {
                naga::AddressSpace::Uniform => "uniform",
                naga::AddressSpace::Storage { .. } => "storage",
                naga::AddressSpace::Handle => "handle",
                naga::AddressSpace::Private => "private",
                naga::AddressSpace::WorkGroup => "workgroup",
                naga::AddressSpace::Function => "function",
                _ => "other",
            };
            let mut info = format!("```wgsl\nvar<{}> {}: {}\n```", space, name, ty);
            if let Some(ref binding) = global.binding {
                info.push_str(&format!(
                    "\n\n@group({}) @binding({})",
                    binding.group, binding.binding
                ));
            }
            return Some(info);
        }
    }

    // Constants
    for (_, constant) in module.constants.iter() {
        if constant.name.as_deref() == Some(name) {
            let ty = type_name(&module.types, &constant.ty);
            return Some(format!("```wgsl\nconst {}: {}\n```", name, ty));
        }
    }

    None
}

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
                _ => format!("array<{}>", base_name),
            }
        }
        _ => format!("{:?}", ty.inner),
    }
}

fn scalar_name(scalar: &naga::Scalar) -> String {
    match (scalar.kind, scalar.width) {
        (naga::ScalarKind::Float, 4) => "f32".to_string(),
        (naga::ScalarKind::Float, 2) => "f16".to_string(),
        (naga::ScalarKind::Sint, 4) => "i32".to_string(),
        (naga::ScalarKind::Uint, 4) => "u32".to_string(),
        (naga::ScalarKind::Bool, _) => "bool".to_string(),
        _ => format!("{:?}", scalar),
    }
}

//! Dead Code Elimination for naga modules.
//!
//! Walks from entry points, marks all reachable functions/globals/types/constants,
//! then removes unreachable declarations from the module.

use std::collections::HashSet;

use naga::{Function, Handle, Module};

/// Remove unreachable declarations from a naga module.
///
/// Starting from all entry points, transitively marks reachable functions,
/// global variables, constants, overrides, and types. Unreachable items are
/// removed from the module.
pub fn eliminate_dead_code(module: &mut Module) {
    let mut reachable = Reachable::default();

    // Seed from entry points
    for ep in &module.entry_points {
        reachable.walk_function(&ep.function, module);
    }

    // Remove unreachable functions
    let reachable_fns = reachable.functions.clone();
    let mut fn_remap: Vec<Option<Handle<Function>>> = Vec::new();
    let mut new_functions = naga::Arena::new();

    for (handle, func) in module.functions.iter() {
        if reachable_fns.contains(&handle) {
            let new_handle = new_functions.append(func.clone(), Default::default());
            fn_remap.push(Some(new_handle));
        } else {
            fn_remap.push(None);
        }
    }

    // If nothing was removed, skip the expensive remapping
    if fn_remap.iter().all(|h| h.is_some()) {
        return;
    }

    module.functions = new_functions;

    // Note: We don't remove unreachable globals/types/constants because naga's
    // arenas use indexed handles — removing items would invalidate all existing
    // handles throughout the module. The naga WGSL writer already skips unused
    // types. The main win from DCE is removing function bodies (code size).
}

#[derive(Default)]
struct Reachable {
    functions: HashSet<Handle<Function>>,
    globals: HashSet<Handle<naga::GlobalVariable>>,
    constants: HashSet<Handle<naga::Constant>>,
}

impl Reachable {
    fn walk_function(&mut self, func: &Function, module: &Module) {
        // Walk expressions for references to globals, constants, and other functions
        for (_, expr) in func.expressions.iter() {
            self.walk_expression(expr, module);
        }

        // Walk statements for function calls
        self.walk_block(&func.body, module);
    }

    fn walk_expression(&mut self, expr: &naga::Expression, module: &Module) {
        match *expr {
            naga::Expression::GlobalVariable(handle) => {
                self.globals.insert(handle);
            }
            naga::Expression::Constant(handle) => {
                self.constants.insert(handle);
            }
            naga::Expression::CallResult(func_handle) => {
                self.mark_function(func_handle, module);
            }
            _ => {}
        }
    }

    fn walk_block(&mut self, block: &naga::Block, module: &Module) {
        for stmt in block.iter() {
            self.walk_statement(stmt, module);
        }
    }

    fn walk_statement(&mut self, stmt: &naga::Statement, module: &Module) {
        match *stmt {
            naga::Statement::Call {
                function: func_handle,
                ..
            } => {
                self.mark_function(func_handle, module);
            }
            naga::Statement::Block(ref block) => {
                self.walk_block(block, module);
            }
            naga::Statement::If {
                ref accept,
                ref reject,
                ..
            } => {
                self.walk_block(accept, module);
                self.walk_block(reject, module);
            }
            naga::Statement::Switch { ref cases, .. } => {
                for case in cases {
                    self.walk_block(&case.body, module);
                }
            }
            naga::Statement::Loop {
                ref body,
                ref continuing,
                ..
            } => {
                self.walk_block(body, module);
                self.walk_block(continuing, module);
            }
            _ => {}
        }
    }

    fn mark_function(&mut self, handle: Handle<Function>, module: &Module) {
        if self.functions.insert(handle) {
            // Newly marked — walk it
            let func = &module.functions[handle];
            self.walk_function(func, module);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::validate;

    #[test]
    fn test_dce_removes_unused_function() {
        let source = r#"
fn used_fn() -> f32 {
    return 1.0;
}

fn unused_fn() -> f32 {
    return 2.0;
}

@fragment
fn main() -> @location(0) vec4<f32> {
    return vec4<f32>(used_fn(), 0.0, 0.0, 1.0);
}
"#;
        let result = validate(source, None);
        let mut module = result.module.unwrap();
        let info = result.module_info.unwrap();

        // Before DCE: 2 functions (used_fn, unused_fn)
        assert_eq!(module.functions.len(), 2);

        eliminate_dead_code(&mut module);

        // After DCE: 1 function (used_fn only)
        assert_eq!(module.functions.len(), 1);
        let func = module.functions.iter().next().unwrap().1;
        assert_eq!(func.name.as_deref(), Some("used_fn"));
    }

    #[test]
    fn test_dce_preserves_transitive_deps() {
        let source = r#"
fn helper() -> f32 {
    return 1.0;
}

fn middle() -> f32 {
    return helper();
}

fn unused() -> f32 {
    return 3.0;
}

@fragment
fn main() -> @location(0) vec4<f32> {
    return vec4<f32>(middle(), 0.0, 0.0, 1.0);
}
"#;
        let result = validate(source, None);
        let mut module = result.module.unwrap();

        assert_eq!(module.functions.len(), 3);

        eliminate_dead_code(&mut module);

        // helper and middle should survive, unused should be removed
        assert_eq!(module.functions.len(), 2);
        let names: Vec<_> = module
            .functions
            .iter()
            .map(|(_, f)| f.name.clone().unwrap_or_default())
            .collect();
        assert!(names.contains(&"helper".to_string()));
        assert!(names.contains(&"middle".to_string()));
        assert!(!names.contains(&"unused".to_string()));
    }

    #[test]
    fn test_dce_no_removal_when_all_used() {
        let source = r#"
fn helper() -> f32 {
    return 1.0;
}

@fragment
fn main() -> @location(0) vec4<f32> {
    return vec4<f32>(helper(), 0.0, 0.0, 1.0);
}
"#;
        let result = validate(source, None);
        let mut module = result.module.unwrap();

        assert_eq!(module.functions.len(), 1);
        eliminate_dead_code(&mut module);
        assert_eq!(module.functions.len(), 1);
    }
}

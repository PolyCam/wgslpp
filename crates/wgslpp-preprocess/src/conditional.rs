use std::collections::HashMap;

use crate::evaluator;

/// State for a single `#if`/`#ifdef`/`#ifndef` nesting level.
#[derive(Debug, Clone)]
struct CondFrame {
    /// Has any branch in this #if chain been taken?
    any_taken: bool,
    /// Is the current branch active (emitting)?
    active: bool,
    /// Was the parent frame active when this frame was entered?
    parent_active: bool,
}

/// Stack machine for conditional preprocessing directives.
#[derive(Debug)]
pub struct ConditionalStack {
    stack: Vec<CondFrame>,
}

impl ConditionalStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Whether code should currently be emitted.
    pub fn is_active(&self) -> bool {
        self.stack.last().map_or(true, |f| f.active)
    }

    /// Process `#ifdef NAME`.
    pub fn ifdef(&mut self, name: &str, defines: &HashMap<String, String>) {
        let parent_active = self.is_active();
        let cond = parent_active && defines.contains_key(name);
        self.stack.push(CondFrame {
            any_taken: cond,
            active: cond,
            parent_active,
        });
    }

    /// Process `#ifndef NAME`.
    pub fn ifndef(&mut self, name: &str, defines: &HashMap<String, String>) {
        let parent_active = self.is_active();
        let cond = parent_active && !defines.contains_key(name);
        self.stack.push(CondFrame {
            any_taken: cond,
            active: cond,
            parent_active,
        });
    }

    /// Process `#if EXPR`.
    pub fn if_expr(
        &mut self,
        expr: &str,
        defines: &HashMap<String, String>,
    ) -> Result<(), String> {
        let parent_active = self.is_active();
        let cond = if parent_active {
            evaluator::evaluate(expr, defines)?
        } else {
            false
        };
        self.stack.push(CondFrame {
            any_taken: cond,
            active: cond,
            parent_active,
        });
        Ok(())
    }

    /// Process `#elif EXPR`.
    pub fn elif(
        &mut self,
        expr: &str,
        defines: &HashMap<String, String>,
    ) -> Result<(), String> {
        let frame = self
            .stack
            .last_mut()
            .ok_or_else(|| "#elif without matching #if".to_string())?;
        if frame.any_taken || !frame.parent_active {
            frame.active = false;
        } else {
            let cond = evaluator::evaluate(expr, defines)?;
            frame.active = cond;
            if cond {
                frame.any_taken = true;
            }
        }
        Ok(())
    }

    /// Process `#else`.
    pub fn else_branch(&mut self) -> Result<(), String> {
        let frame = self
            .stack
            .last_mut()
            .ok_or_else(|| "#else without matching #if".to_string())?;
        if frame.any_taken || !frame.parent_active {
            frame.active = false;
        } else {
            frame.active = true;
            frame.any_taken = true;
        }
        Ok(())
    }

    /// Process `#endif`.
    pub fn endif(&mut self) -> Result<(), String> {
        self.stack
            .pop()
            .ok_or_else(|| "#endif without matching #if".to_string())?;
        Ok(())
    }

    /// Check that all conditionals are closed.
    pub fn check_balanced(&self) -> Result<(), String> {
        if self.stack.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "unterminated conditional block ({} unclosed #if/#ifdef/#ifndef)",
                self.stack.len()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_ifdef_active() {
        let d = defs(&[("FOO", "")]);
        let mut cs = ConditionalStack::new();
        assert!(cs.is_active());
        cs.ifdef("FOO", &d);
        assert!(cs.is_active());
        cs.endif().unwrap();
        assert!(cs.is_active());
    }

    #[test]
    fn test_ifdef_inactive() {
        let d = defs(&[]);
        let mut cs = ConditionalStack::new();
        cs.ifdef("FOO", &d);
        assert!(!cs.is_active());
        cs.endif().unwrap();
    }

    #[test]
    fn test_else() {
        let d = defs(&[]);
        let mut cs = ConditionalStack::new();
        cs.ifdef("FOO", &d);
        assert!(!cs.is_active());
        cs.else_branch().unwrap();
        assert!(cs.is_active());
        cs.endif().unwrap();
    }

    #[test]
    fn test_nested() {
        let d = defs(&[("A", "")]);
        let mut cs = ConditionalStack::new();
        cs.ifdef("A", &d);
        assert!(cs.is_active());
        cs.ifdef("B", &d);
        assert!(!cs.is_active()); // B not defined, parent active but condition false
        cs.endif().unwrap();
        assert!(cs.is_active()); // back to A's scope
        cs.endif().unwrap();
    }

    #[test]
    fn test_elif() {
        let d = defs(&[("X", "2")]);
        let mut cs = ConditionalStack::new();
        cs.if_expr("X == 1", &d).unwrap();
        assert!(!cs.is_active());
        cs.elif("X == 2", &d).unwrap();
        assert!(cs.is_active());
        cs.elif("X == 3", &d).unwrap();
        assert!(!cs.is_active()); // already taken
        cs.else_branch().unwrap();
        assert!(!cs.is_active()); // already taken
        cs.endif().unwrap();
    }

    #[test]
    fn test_unbalanced() {
        let mut cs = ConditionalStack::new();
        cs.ifdef("X", &defs(&[]));
        assert!(cs.check_balanced().is_err());
    }
}

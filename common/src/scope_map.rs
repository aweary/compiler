// use diagnostics::result::Result;
use std::collections::HashMap;
// use syntax::ast::*;
// use syntax::symbol::Symbol;

use std::sync::Arc;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct ScopeId(u32);

#[derive(Debug)]
pub enum Binding {
    Let(Arc<Let>),
}

#[derive(Default)]
struct Scope {
    id: u32,
    bindings: HashMap<Symbol, Binding>,
}

pub struct ScopeMap {
    unique_id: u32,
    scopes: Vec<Scope>,
}

impl Default for ScopeMap {
    fn default() -> Self {
        ScopeMap {
            unique_id: 0,
            scopes: vec![Scope::default()],
        }
    }
}

impl ScopeMap {
    // TODO ExpressionId isn't going to be sufficient. How do we deal with references
    // to types? imports?
    pub fn define(&mut self, identifer: Symbol, binding: Binding) -> UniqueName {
        self.scopes
            .first_mut()
            .unwrap()
            .bindings
            .insert(identifer, binding);
        let id = self.unique_id;
        self.unique_id += 1;
        id.into()
    }
    pub fn resolve(&mut self, identifer: &Symbol) -> Option<&Binding> {
        self.scopes.first().unwrap().bindings.get(identifer)
    }

    pub fn unique_name(&mut self) -> UniqueName {
        let id = self.unique_id;
        self.unique_id += 1;
        id.into()
    }
}

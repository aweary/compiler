// use diagnostics::result::Result;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

pub trait Reference: Debug + Eq + Hash + Clone {}
pub trait Referant: Debug + Eq + Clone {}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueReference<K: Reference>(u16, PhantomData<K>);

impl<T: Reference> Copy for UniqueReference<T> {}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct ScopeId(u32);

/// An individual scope. Mapped to a block or module, as those are the
/// only language items that allow for scope creation.
#[derive(Debug)]
pub struct Scope<K: Reference, V: Referant> {
    bindings: HashMap<K, (V, UniqueReference<K>)>,
}

impl<K: Reference, V: Referant> Default for Scope<K, V> {
    fn default() -> Self {
        Scope {
            bindings: HashMap::default(),
        }
    }
}

impl<K: Reference, V: Referant> Scope<K, V> {
    pub fn define(&mut self, reference: K, referant: V, unique_reference: UniqueReference<K>) {
        self.bindings
            .insert(reference, (referant, unique_reference));
    }
    pub fn resolve(&mut self, reference: &K) -> Option<(V, UniqueReference<K>)> {
        self.bindings.get(&reference).cloned()
    }
}

pub struct ScopeMap<K: Reference, V: Referant> {
    unique_id: u16,
    scopes: Vec<Scope<K, V>>,
}

impl<K: Reference, V: Referant> Default for ScopeMap<K, V> {
    fn default() -> Self {
        ScopeMap {
            unique_id: 0,
            scopes: vec![
                // ScopeMap has a default scope, which ends up
                // being the top-level module scope since we create
                // a New `Parser` instance for each module.
                Scope::default(),
            ],
        }
    }
}

impl<K: Reference, V: Referant> ScopeMap<K, V> {
    fn generate_unique_reference(&mut self) -> UniqueReference<K> {
        let id = self.unique_id;
        self.unique_id += 1;
        UniqueReference(id, PhantomData)
    }

    pub fn extend(&mut self) {
        self.scopes.push(Scope::default())
    }

    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    // TODO ExpressionId isn't going to be sufficient. How do we deal with references
    // to types? imports?
    pub fn define(&mut self, identifer: K, binding: V) -> UniqueReference<K> {
        let unique_reference = self.generate_unique_reference();
        self.scopes
            .first_mut()
            .unwrap()
            .bindings
            .insert(identifer, (binding, unique_reference));
        self.unique_id += 1;
        unique_reference
    }
    pub fn resolve(&mut self, identifer: &K) -> Option<&(V, UniqueReference<K>)> {
        self.scopes.first().unwrap().bindings.get(identifer)
    }
}

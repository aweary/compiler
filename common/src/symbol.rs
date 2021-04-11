use crate::scope_map::Reference;
use std::fmt::{Debug, Display};
use std::sync::Mutex;
use std::{collections::HashMap, mem};

thread_local! {
    pub static SYMBOL_INTERNER : Mutex<SymbolInterner> = Mutex::new(SymbolInterner::default())
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(u32);

impl Reference for Symbol {}

impl Symbol {
    pub fn intern(name: &str) -> Symbol {
        SYMBOL_INTERNER.with(|interner| {
            let mut gaurd = interner.lock().unwrap();
            gaurd.intern(name)
        })
    }
}

impl Into<f64> for Symbol {
    fn into(self) -> f64 {
        SYMBOL_INTERNER.with(|interner| {
            let interner = interner.lock().unwrap();
            // TODO(aweary) is this really where we should strip the separator characters?
            let string = interner.lookup(self).replace("_", "");
            string.parse::<f64>().unwrap()
        })
    }
}

impl Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        SYMBOL_INTERNER.with(|interner| {
            let interner = interner.lock().unwrap();
            let string = interner.lookup(*self);
            write!(f, "{}", string)
        })
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        SYMBOL_INTERNER.with(|interner| {
            let interner = interner.lock().unwrap();
            let string = interner.lookup(*self);
            write!(f, "{}", string)
        })
    }
}

pub struct SymbolInterner {
    map: HashMap<&'static str, Symbol>,
    vec: Vec<&'static str>,
    buf: String,
    full: Vec<String>,
}

impl Default for SymbolInterner {
    fn default() -> Self {
        SymbolInterner::with_capacity(2 ^ 8)
    }
}

impl SymbolInterner {
    pub fn with_capacity(cap: usize) -> SymbolInterner {
        let cap = cap.next_power_of_two();
        SymbolInterner {
            map: HashMap::default(),
            vec: Vec::new(),
            buf: String::with_capacity(cap),
            full: Vec::new(),
        }
    }

    pub fn intern(&mut self, name: &str) -> Symbol {
        if let Some(&id) = self.map.get(name) {
            return id;
        }
        let name = unsafe { self.alloc(name) };
        let id = Symbol(self.map.len() as u32);
        self.map.insert(name, id);
        self.vec.push(name);

        debug_assert!(self.lookup(id) == name);
        debug_assert!(self.intern(name) == id);

        id
    }

    pub fn lookup(&self, id: Symbol) -> &str {
        self.vec[id.0 as usize]
    }

    unsafe fn alloc(&mut self, name: &str) -> &'static str {
        let cap = self.buf.capacity();
        if cap < self.buf.len() + name.len() {
            let new_cap = (cap.max(name.len()) + 1).next_power_of_two();
            let new_buf = String::with_capacity(new_cap);
            let old_buf = mem::replace(&mut self.buf, new_buf);
            self.full.push(old_buf);
        }

        let interned = {
            let start = self.buf.len();
            self.buf.push_str(name);
            &self.buf[start..]
        };

        &*(interned as *const str)
    }
}

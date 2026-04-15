use std::ops::Index;

use crate::types::string::RbStr;

/// Index into a [`SymbolTable`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolIdx(usize);

impl SymbolIdx {
    pub fn inner(&self) -> usize {
        self.0
    }

    pub fn new(idx: usize) -> Self {
        Self(idx)
    }
}

/// Table of all symbols collected from a marshal stream
#[derive(Debug, Clone, Default)]
pub struct SymbolTable<'a> {
    symbols: Vec<&'a RbStr>,
}

impl<'a> SymbolTable<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, idx: SymbolIdx) -> Option<&'a RbStr> {
        self.symbols.get(idx.inner()).copied()
    }

    pub fn push(&mut self, symbol: &'a RbStr) -> SymbolIdx {
        let idx = SymbolIdx::new(self.symbols.len());
        self.symbols.push(symbol);
        idx
    }
}

impl<'a> Index<usize> for SymbolTable<'a> {
    type Output = RbStr;

    fn index(&self, index: usize) -> &Self::Output {
        self.symbols.index(index)
    }
}

impl<'a> Index<SymbolIdx> for SymbolTable<'a> {
    type Output = RbStr;

    fn index(&self, index: SymbolIdx) -> &Self::Output {
        self.symbols.index(index.inner())
    }
}

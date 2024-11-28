use crate::collections::id_map::IdMap;
use crate::core::{u32_to_cst, IntCst};
use crate::create_ref_type;
use crate::model::types::{TypeHierarchy, TypeId};
use anyhow::*;
use std::collections::HashMap;
use std::fmt::{self, Write};
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

use crate::collections::ref_store::RefVec;
use crate::utils::input::Sym;
use std::borrow::Borrow;

/// Associates each symbol (of rust type `Sym`) to
///  - its type (represented as the rust type `T`
///  - a `SymId` that is an unique numeric representation of symbol aimed
///    at performance : low footprint, usable as array index and cheap comparison
#[derive(Clone)]
pub struct SymbolTable {
    pub types: TypeHierarchy,
    // TODO: use a RefStore
    pub symbols: Vec<Sym>,
    ids: HashMap<Sym, SymId>,
    symbol_types: RefVec<SymId, TypeId>,
    instances_by_exact_type: IdMap<TypeId, ContiguousSymbols>,
}

impl Debug for SymbolTable {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, x) in self.symbols.iter().enumerate() {
            writeln!(f, "{:?}\t<- {:?}", SymId::from(i), x)?;
        }
        std::fmt::Result::Ok(())
    }
}

/// An iterable structure representing a contiguous sequence of symbols.
/// It is typically used to represent all instances of a given type.
#[derive(Copy, Clone, Debug)]
pub struct ContiguousSymbols {
    first: usize,
    after_last: usize,
}

impl ContiguousSymbols {
    pub fn new(first: SymId, last: SymId) -> Self {
        let last_id: usize = last.into();
        ContiguousSymbols {
            first: first.into(),
            after_last: last_id + 1,
        }
    }

    pub fn singleton(item: SymId) -> Self {
        ContiguousSymbols::new(item, item)
    }

    pub fn size(self) -> u32 {
        (self.after_last - self.first).max(0) as u32
    }

    /// Returns the first and last element of these instances.
    /// If the interval is empty, returns None.
    pub fn bounds(self) -> Option<(SymId, SymId)> {
        if self.after_last > self.first {
            Some((self.first.into(), (self.after_last - 1).into()))
        } else {
            None
        }
    }

    /// If there is exactly one symbol in the symbol set, returns it.
    /// Returns None otherwise.
    pub fn into_singleton(self) -> Option<SymId> {
        if self.first == self.after_last - 1 {
            Some(self.first.into())
        } else {
            None
        }
    }

    pub fn contains(self, sym: SymId) -> bool {
        let sym = usize::from(sym);
        self.first <= sym && sym < self.after_last
    }
}

impl Iterator for ContiguousSymbols {
    type Item = SymId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.first < self.after_last {
            self.first += 1;
            Some(SymId::from(self.first - 1))
        } else {
            None
        }
    }
}

impl SymbolTable {
    pub fn empty() -> Self {
        let th = TypeHierarchy::new(Vec::new()).unwrap();
        Self::new(th, Vec::new()).unwrap()
    }

    /// Constructs a new symbol table from a type hierarchy and set of pairs `(symbol, type)`
    pub fn new(th: TypeHierarchy, symbols: Vec<(Sym, Sym)>) -> Result<Self> {
        let mut instances_by_type = HashMap::new();
        for (sym, tpe) in symbols {
            let tpe_id = th.id_of(&tpe).ok_or_else(|| tpe.invalid("Unknown atom"))?;
            instances_by_type
                .entry(tpe_id)
                .or_insert_with(|| Vec::with_capacity(1))
                .push(sym);
        }

        let mut table = SymbolTable {
            types: th,
            symbols: Default::default(),
            ids: Default::default(),
            symbol_types: Default::default(),
            instances_by_exact_type: Default::default(),
        };

        for tpe in table.types.types() {
            let first = table.symbols.len();

            for sym in instances_by_type.remove(&tpe).unwrap_or_default() {
                if let Some(&sym_id) = table.ids.get(&sym) {
                    ensure!(
                        table.symbol_types[sym_id] == tpe,
                        "Duplicated instance with different types"
                    );
                    tracing::warn!("duplicated instance : {}", sym);
                    continue;
                }

                let id = SymId::from(table.symbols.len());
                table.symbols.push(sym.clone());
                table.ids.insert(sym, id);
                let sym_alias = table.symbol_types.push(tpe);
                assert_eq!(id, sym_alias, "Problem in the insertion order");
            }
            let after_last = table.symbols.len();
            table
                .instances_by_exact_type
                .insert(tpe, ContiguousSymbols { first, after_last });
        }

        Result::Ok(table)
    }

    /// Retrieves the ID of a given symbol. Return None if the symbol doesn't appear in the
    /// symbol table.
    pub fn id<W>(&self, sym: &W) -> Option<SymId>
    where
        W: Eq + Hash + ?Sized,
        Sym: Eq + Hash + Borrow<W>,
    {
        self.ids.get(sym).copied()
    }

    /// Returns the symbol associated to the given ID.
    pub fn symbol(&self, id: SymId) -> &Sym {
        let i: usize = id.into();
        &self.symbols[i]
    }

    /// Returns the type of the symbol
    pub fn type_of(&self, id: SymId) -> TypeId {
        self.symbol_types[id]
    }

    /// Returns an iterator on all symbols in the table.
    pub fn iter(&self) -> ContiguousSymbols {
        ContiguousSymbols::new(SymId::from(0usize), (self.symbols.len() - 1).into())
    }

    /// Returns an iterator on all direct or indirect instances of the given type
    pub fn instances_of_type(&self, tpe: TypeId) -> ContiguousSymbols {
        let mut instance = self.instances_by_exact_type[tpe];
        instance.after_last = self.instances_by_exact_type[self.types.last_subtype(tpe)].after_last;
        instance
    }

    /// Returns a formated view of an S-Expression
    pub fn format<'a>(&self, sexpr: impl IntoIterator<Item = &'a SymId>) -> String
    where
        Sym: Display,
    {
        let mut s = String::from("(");
        for sym in sexpr {
            write!(s, "{} ", self.symbol(*sym)).unwrap();
        }
        if s.ends_with(' ') {
            s.pop();
        }
        s.push(')');
        s
    }
}

create_ref_type!(SymId);

impl SymId {
    pub fn int_value(self) -> IntCst {
        u32_to_cst(u32::from(self))
    }
}

impl Debug for SymId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "sym{}", usize::from(*self))
    }
}

#[derive(Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct TypedSym {
    pub sym: SymId,
    pub tpe: TypeId,
}

impl Debug for TypedSym {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:?}", self.sym)
    }
}

impl TypedSym {
    pub fn new(sym: SymId, tpe: TypeId) -> Self {
        TypedSym { sym, tpe }
    }
}

impl From<TypedSym> for SymId {
    fn from(ts: TypedSym) -> Self {
        ts.sym
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::model::types::TypeHierarchy;
    use crate::utils::enumerate;
    use streaming_iterator::StreamingIterator;

    #[test]
    fn instances() {
        let symbols = table();
        let types = &symbols.types;
        let rover = types.id_of("rover").unwrap();
        let predicate = types.id_of("predicate").unwrap();
        let location = types.id_of("location").unwrap();
        let object = types.id_of("object").unwrap();
        assert_eq!(symbols.instances_of_type(predicate).count(), 2);
        assert_eq!(symbols.instances_of_type(object).count(), 3);
        assert_eq!(symbols.instances_of_type(rover).count(), 1);
        assert_eq!(symbols.instances_of_type(location).count(), 2);
    }

    #[test]
    fn enumeration() {
        let symbols = table();
        let types = &symbols.types;
        let rover = types.id_of("rover").unwrap();
        let predicate = types.id_of("predicate").unwrap();
        let location = types.id_of("location").unwrap();
        let _object = types.id_of("object").unwrap();
        let x = [
            symbols.instances_of_type(predicate),
            symbols.instances_of_type(rover),
            symbols.instances_of_type(location),
            symbols.instances_of_type(location),
        ];

        let mut xx = enumerate(x.to_vec());

        while let Some(comb) = xx.next() {
            println!("{comb:?}")
        }
        println!("DONE")
    }

    pub fn table() -> SymbolTable {
        let types = vec![
            ("predicate".into(), None),
            ("object".into(), None),
            ("rover".into(), Some("object".into())),
            ("location".into(), Some("object".into())),
        ];
        let types = TypeHierarchy::new(types).unwrap();

        let instances = vec![
            ("at".into(), "predicate".into()),
            ("can_traverse".into(), "predicate".into()),
            ("rover1".into(), "rover".into()),
            ("l1".into(), "location".into()),
            ("l2".into(), "location".into()),
        ];

        SymbolTable::new(types, instances).unwrap()
    }
}

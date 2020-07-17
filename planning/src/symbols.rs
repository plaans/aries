use crate::typesystem::{TypeHierarchy, TypeId};
use anyhow::*;
use aries_collections::id_map::IdMap;
use std::collections::HashMap;
use std::fmt::Write;
use std::fmt::{Debug, Display, Error, Formatter};
use std::hash::Hash;

use std::borrow::Borrow;

/// Associates each symbol (of rust type `Sym`) to
///  - its type (represented as the rust type `T`
///  - a `SymId` that is an unique numeric representation of symbol aimed
///    at performance : low footprint, usable as array index and cheap comparison
#[derive(Clone)]
pub struct SymbolTable<T, Sym> {
    pub types: TypeHierarchy<T>,
    // TODO: use a RefStore
    symbols: Vec<Sym>,
    ids: HashMap<Sym, SymId>,
    instances_by_exact_type: IdMap<TypeId, ContiguousSymbols>,
}

impl<T, Sym: Debug> Debug for SymbolTable<T, Sym> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        for (i, x) in self.symbols.iter().enumerate() {
            writeln!(f, "{:?}\t<- {:?}", SymId::from(i), x)?;
        }
        Ok(())
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

impl<T, Sym> SymbolTable<T, Sym> {
    /// Constructs a new symbol table from a type hierachy and set of pairs `(symbol, type)`
    pub fn new(th: TypeHierarchy<T>, symbols: Vec<(Sym, T)>) -> Result<Self>
    where
        T: Clone + Eq + Hash,
        Sym: Clone + Eq + Hash + Display,
    {
        let mut instances_by_type = HashMap::new();
        for (sym, tpe) in symbols {
            let tpe_id = th.id_of(&tpe).unwrap();
            instances_by_type
                .entry(tpe_id)
                .or_insert_with(|| Vec::with_capacity(1))
                .push(sym);
        }

        let mut table = SymbolTable {
            types: th,
            symbols: Default::default(),
            ids: Default::default(),
            instances_by_exact_type: Default::default(),
        };

        for tpe in table.types.types() {
            let first = table.symbols.len();

            for sym in instances_by_type.remove(&tpe).unwrap_or_default() {
                ensure!(!table.ids.contains_key(&sym), "duplicated instance : {}", sym);

                let id = SymId::from(table.symbols.len());
                table.symbols.push(sym.clone());
                table.ids.insert(sym, id);
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
    pub fn id<W: ?Sized>(&self, sym: &W) -> Option<SymId>
    where
        W: Eq + Hash,
        Sym: Eq + Hash + Borrow<W>,
    {
        self.ids.get(sym).copied()
    }

    /// Returns the symbol associated to the given ID.
    pub fn symbol(&self, id: SymId) -> &Sym {
        let i: usize = id.into();
        &self.symbols[i]
    }

    /// Returns an iterator on all symbols in the table.
    pub fn iter(&self) -> ContiguousSymbols {
        ContiguousSymbols::new(0.into(), (self.symbols.len() - 1).into())
    }

    /// Returns an iterator on all direct or indirect instances of the given type
    pub fn instances_of_type(&self, tpe: TypeId) -> ContiguousSymbols {
        let mut instance = self.instances_by_exact_type[tpe];
        instance.after_last = self.instances_by_exact_type[self.types.last_subtype(tpe)].after_last;
        instance
    }

    /// Returns a formated view of an S-Expression
    pub fn format(&self, sexpr: &[SymId]) -> String
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

/// Numeric representation of a Symbol.
/// The SymId is typically associated to a SymbolTable that created it.
///
/// TODO: use macro to generate a ref compatible type that forbids zero value
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash)]
pub struct SymId(u32);

impl Into<usize> for SymId {
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl From<usize> for SymId {
    fn from(i: usize) -> Self {
        SymId(i as u32)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::utils::enumerate;
    use streaming_iterator::StreamingIterator;

    #[test]
    fn instances() {
        let symbols = table();
        let types = &symbols.types;
        let rover = types.id_of(&"rover").unwrap();
        let predicate = types.id_of(&"predicate").unwrap();
        let location = types.id_of(&"location").unwrap();
        let object = types.id_of(&"object").unwrap();
        assert_eq!(symbols.instances_of_type(predicate).count(), 2);
        assert_eq!(symbols.instances_of_type(object).count(), 3);
        assert_eq!(symbols.instances_of_type(rover).count(), 1);
        assert_eq!(symbols.instances_of_type(location).count(), 2);
    }

    #[test]
    fn enumeration() {
        let symbols = table();
        let types = &symbols.types;
        let rover = types.id_of(&"rover").unwrap();
        let predicate = types.id_of(&"predicate").unwrap();
        let location = types.id_of(&"location").unwrap();
        let _object = types.id_of(&"object").unwrap();
        let x = [
            symbols.instances_of_type(predicate),
            symbols.instances_of_type(rover),
            symbols.instances_of_type(location),
            symbols.instances_of_type(location),
        ];

        let mut xx = enumerate(x.to_vec());

        while let Some(comb) = xx.next() {
            println!("{:?}", comb)
        }
        println!("DONE")
    }

    pub fn table() -> SymbolTable<&'static str, &'static str> {
        let types = vec![
            ("predicate", None),
            ("object", None),
            ("rover", Some("object")),
            ("location", Some("object")),
        ];
        let types = TypeHierarchy::new(types).unwrap();

        let instances = vec![
            ("at", "predicate"),
            ("can_traverse", "predicate"),
            ("rover1", "rover"),
            ("l1", "location"),
            ("l2", "location"),
        ];
        let symbols = SymbolTable::new(types.clone(), instances).unwrap();
        symbols
    }
}

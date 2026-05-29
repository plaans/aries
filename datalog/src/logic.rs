use core::todo;
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

use crate::*;

/// Representation of a symbol in a datalog program.
pub type Sym = u32;

type Tuple<E, const N: usize> = [E; N];

pub(crate) type Fact<const N: usize> = Tuple<Sym, N>;

/// A buffer of rows, with no guarantees on order or redundancy.
pub(crate) struct TableBuff<T> {
    /// Number of columns in each row.
    num_columns: usize,
    /// Flattened vector of rows.
    data: Vec<T>,
}

impl<T> TableBuff<T> {
    pub fn new(num_columns: usize) -> Self {
        Self {
            num_columns,
            data: Default::default(),
        }
    }

    pub fn push(&mut self, row: &[T])
    where
        T: Clone,
    {
        assert_eq!(row.len(), self.num_columns);
        self.data.extend_from_slice(row);
    }

    pub fn extend<'me, 'rows>(&'me mut self, rows: impl IntoIterator<Item = &'rows [T]>)
    where
        T: Clone + 'static,
    {
        for row in rows {
            self.push(row);
        }
    }

    pub fn rows(&self) -> impl Iterator<Item = &[T]> + '_ {
        self.data.chunks(self.num_columns)
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl TableBuff<Sym> {
    pub fn move_to_table(&mut self) -> Table {
        let data = std::mem::take(&mut self.data);
        Table::new_from_flat(self.num_columns, data)
    }
}

/// A table with rows of fixed size, that are ordered and unique.
pub struct Table {
    /// Number of elements in each row
    num_columns: usize,
    /// This is a flattened representation of `Vec<[Sym; num_columns]>`
    data: Vec<Sym>,
}

impl Table {
    /// Creates a new empty table with the given number of columns.
    pub fn new_empty(num_columns: usize) -> Self {
        Self {
            num_columns,
            data: Vec::new(),
        }
    }

    /// Creates a new table with the given rows.
    pub fn new<const N: usize>(mut data: Vec<[Sym; N]>) -> Table {
        const { assert!(N != 0) }
        data.sort_unstable();
        data.dedup();
        Table {
            num_columns: N,
            data: data.into_flattened(),
        }
    }

    /// Creates a new table from a flattened vector of rows.
    pub fn new_from_flat(num_columns: usize, data: Vec<Sym>) -> Table {
        match num_columns {
            0 => panic!(
                "Table with no columns in not supported (due to unnatural flattened representation)"
            ),
            1 => Table::new(Self::into_chunks::<1>(data)),
            2 => Table::new(Self::into_chunks::<2>(data)),
            3 => Table::new(Self::into_chunks::<3>(data)),
            4 => Table::new(Self::into_chunks::<4>(data)),
            5 => Table::new(Self::into_chunks::<5>(data)),
            _ => todo!(),
        }
    }

    /// Returns true if the table is empty (has no rows)
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Extends this table with the rows of another table.
    pub(crate) fn extend(&mut self, other: &Table) {
        assert_eq!(self.num_columns, other.num_columns);
        let mut base = std::mem::take(&mut self.data);
        base.extend_from_slice(&other.data);
        let mut table = Table::new_from_flat(self.num_columns, base);
        std::mem::swap(&mut table.data, &mut self.data);
    }

    /// Returns a view of all rows, with a size known at compile time.
    ///
    /// Panics if `N` does not match the number of columns.
    pub fn rows_sized<const N: usize>(&self) -> &[[Sym; N]] {
        assert_eq!(self.num_columns, N);
        self.data.as_chunks().0
    }

    /// Returns an iterator over all rows in the table.
    ///
    /// See [`Self::rows_sized`] when the number of columns is know at compile time.
    pub fn rows(&self) -> impl Iterator<Item = &[Sym]> {
        self.data.chunks(self.num_columns)
    }

    /// Implementation copied and adapted from the the unstable `Vec::into_chunks` method in stdlib.
    fn into_chunks<const N: usize>(mut vec: Vec<Sym>) -> Vec<[Sym; N]> {
        const {
            assert!(N != 0, "chunk size must be greater than zero");
        }

        assert_eq!(vec.len() % N, 0);

        vec.shrink_to_fit();
        assert_eq!(vec.len(), vec.capacity());

        let (ptr, len, cap) = vec.into_raw_parts();

        // SAFETY:
        // - `ptr` and `alloc` were just returned from `self.into_raw_parts_with_alloc()`
        // - `[T; N]` has the same alignment as `T`
        // - `size_of::<[T; N]>() * cap / N == size_of::<T>() * cap`
        // - `len / N <= cap / N` because `len <= cap`
        // - the allocated memory consists of `len / N` valid values of type `[T; N]`
        // - `cap / N` fits the size of the allocated memory after shrinking
        unsafe { Vec::from_raw_parts(ptr.cast(), len / N, cap / N) }
    }

    /// Retains only elements of `self` that do not appear in `reference`
    fn retain_distinct(&mut self, reference: &Table) {
        match self.num_columns {
            1 => self.retain_distinct_spec::<1>(reference),
            2 => self.retain_distinct_spec::<2>(reference),
            3 => self.retain_distinct_spec::<3>(reference),
            4 => self.retain_distinct_spec::<4>(reference),
            5 => self.retain_distinct_spec::<5>(reference),
            _ => todo!(),
        }
    }
    /// Arity-specialized version of [`Self::retain_distinct`]
    fn retain_distinct_spec<const N: usize>(&mut self, reference: &Table) {
        let data = std::mem::take(&mut self.data);
        let mut data = Self::into_chunks::<N>(data);
        let mut reference = reference.rows_sized::<N>();
        data.retain(move |row| {
            while !reference.is_empty() && &reference[0] < row {
                reference = &reference[1..];
            }
            reference.is_empty() || &reference[0] != row
        });
        self.data = data.into_flattened();
    }
}

/// A table to which elements can be added, either manually or as a result of running the program.
///
/// A `VarTable` should only be created through a [`Program::new_predicate()`] to ensure that it is correctly accounted
/// for when determining whether inference has reached a fixed point.
///
/// After running [`Program::run()`], all `VarTable`s will have all facts added to it, which can then be retrieved with [`VarTable::extract()`].
#[derive(Clone)]
pub struct VarTable {
    /// Stable set, containing rows that were added at least two iterations ago.
    /// All rows will eventually make it to this set.
    pub(crate) stable: Rc<RefCell<Table>>,
    /// Rows that were added in the previous iteration of the program.
    ///
    /// Rows are deduplicated and not present int `stable`
    pub(crate) recent: Rc<RefCell<Table>>,
    /// Rows that have been produced in the current iteration.
    /// Potentially contains redundancies.
    pub(crate) to_add: Rc<RefCell<TableBuff<Sym>>>,
}

impl VarTable {
    pub(crate) fn new(num_columns: usize) -> Self {
        Self {
            stable: Rc::new(RefCell::new(Table::new_empty(num_columns))),
            recent: Rc::new(RefCell::new(Table::new_empty(num_columns))),
            to_add: Rc::new(RefCell::new(TableBuff::new(num_columns))),
        }
    }

    #[cfg(test)]
    pub(crate) fn from<const N: usize>(rows: impl AsRef<[[Sym; N]]>) -> Self {
        let var = VarTable::new(N);
        var.extend(rows.as_ref().iter().map(|row| row.as_slice()));
        var
    }

    /// Number of columns in the table (i.e. arity of the associated predicate)
    pub fn arity(&self) -> usize {
        self.stable.borrow().num_columns
    }

    /// Adds a row to the table.
    pub fn add(&self, row: impl AsRef<[Sym]>) {
        self.to_add.borrow_mut().push(row.as_ref());
    }

    /// Adds several rows into the table.
    pub fn extend<'a>(&self, rows: impl IntoIterator<Item = &'a [Sym]>) {
        self.to_add.borrow_mut().extend(rows)
    }

    /// Creates a [`RuleAtom`], that can then be used to construct [`Rule`]s.
    ///
    /// ```
    /// use aries_datalog::*;
    /// let mut prog = Program::new();
    /// let parent = prog.new_predicate(2);
    /// // ...
    /// let atom: RuleAtom = parent.apply([Arg::Var(0), Arg::Var(1)]);
    /// use Arg::*; // for shorter notation
    /// let atom: RuleAtom = parent.apply([Var(0), Var(1)]);
    /// ```
    pub fn apply(&self, args: impl AsRef<[Arg]>) -> RuleAtom {
        RuleAtom::new(self.clone(), args)
    }

    /// Returns true if the variable table has no unprocessed elements.
    ///
    /// Elements must be processed with [`Program::run()`].
    pub fn stable(&self) -> bool {
        self.recent.borrow().is_empty() && self.to_add.borrow().is_empty()
    }

    /// Returns the table with all elements in the relation.
    ///
    /// Will panic if the variable has unprocessed elements.
    pub fn extract<'me>(&'me self) -> Ref<'me, Table> {
        assert!(
            self.stable(),
            "VarTable has unprocessed elements, the program likely did run to completion."
        );
        self.stable.borrow()
    }

    pub(crate) fn process(&self) {
        // move recent to stable
        self.stable.borrow_mut().extend(&self.recent.borrow());

        // create table of the elements to add (sorted with no duplicates)
        let mut new = self.to_add.borrow_mut().move_to_table();
        // remove from new elements all those already in stable
        new.retain_distinct(&self.stable.borrow());
        // copy those in the recent set (erasing all the recent one that were move to stable)
        *self.recent.borrow_mut() = new;
    }
}

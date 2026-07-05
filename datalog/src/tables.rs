use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

use crate::*;

/// Representation of a symbol in a datalog program.
pub type Sym = u32;

type Tuple<E, const N: usize> = [E; N];

pub(crate) type Fact<const N: usize> = Tuple<Sym, N>;

// Nullary (0-arity) predicates are represented separately to keep the non-nullary (arity > 0)
// representation invariants simple. A nullary "table" can only ever contain
// the empty tuple, so its content is just a single `bool` flag.

/// A buffer for nullary rows. Tracks whether the empty tuple has been pushed.
pub(crate) struct NullaryBuff {
    pushed: bool,
}

impl NullaryBuff {
    pub fn new() -> Self {
        Self { pushed: false }
    }

    /// Pushes the empty tuple.
    pub fn push(&mut self) {
        self.pushed = true;
    }

    pub fn is_empty(&self) -> bool {
        !self.pushed
    }

    /// Moves the buffer content into a [`NullaryTable`], resetting the buffer.
    pub fn move_to_table(&mut self) -> NullaryTable {
        let has_row = std::mem::take(&mut self.pushed);
        NullaryTable { has_row }
    }
}

/// A table of arity 0: contains at most the empty tuple `()`.
pub struct NullaryTable {
    has_row: bool,
}

impl NullaryTable {
    /// Creates a new empty nullary table (does not contain the empty tuple).
    pub fn new_empty() -> Self {
        Self { has_row: false }
    }

    /// Returns true if the table contains the empty tuple.
    pub fn has_row(&self) -> bool {
        self.has_row
    }

    /// Returns true if the table is empty.
    pub fn is_empty(&self) -> bool {
        !self.has_row
    }

    /// Merges `recent` into `self` (logical OR).
    fn merge(&mut self, recent: NullaryTable) {
        self.has_row = self.has_row || recent.has_row;
    }

    /// Removes the row from `self` if it is present in `reference`.
    fn retain_distinct(&mut self, reference: &NullaryTable) {
        if reference.has_row {
            self.has_row = false;
        }
    }
}

/// Either a non-nullary or nullary table.
pub(crate) enum TableRepr {
    /// A table over a predicate of arity >= 1.
    NonNullary(Table),
    /// A table over a predicate of arity 0.
    Nullary(NullaryTable),
}

impl TableRepr {
    /// Returns true if the table contains no rows.
    pub fn is_empty(&self) -> bool {
        match self {
            TableRepr::NonNullary(t) => t.is_empty(),
            TableRepr::Nullary(t) => t.is_empty(),
        }
    }

    fn new_empty(num_columns: usize) -> Self {
        if num_columns == 0 {
            TableRepr::Nullary(NullaryTable::new_empty())
        } else {
            TableRepr::NonNullary(Table::new_empty(num_columns))
        }
    }

    fn merge(&mut self, recent: TableRepr) {
        match (self, recent) {
            (TableRepr::NonNullary(a), TableRepr::NonNullary(b)) => a.merge(b),
            (TableRepr::Nullary(a), TableRepr::Nullary(b)) => a.merge(b),
            _ => panic!("attempted to merge tables with mismatched arities"),
        }
    }

    fn retain_distinct(&mut self, reference: &TableRepr) {
        match (self, reference) {
            (TableRepr::NonNullary(a), TableRepr::NonNullary(b)) => a.retain_distinct(b),
            (TableRepr::Nullary(a), TableRepr::Nullary(b)) => a.retain_distinct(b),
            _ => panic!("attempted to retain_distinct on tables with mismatched arities"),
        }
    }
}

/// Storage for either a non-nullary or a nullary buffer.
pub(crate) enum BuffRepr {
    /// A buffer over a predicate of arity >= 1.
    NonNullary(TableBuff<Sym>),
    /// A buffer over a predicate of arity 0.
    Nullary(NullaryBuff),
}

impl BuffRepr {
    fn new(num_columns: usize) -> Self {
        if num_columns == 0 {
            BuffRepr::Nullary(NullaryBuff::new())
        } else {
            BuffRepr::NonNullary(TableBuff::new(num_columns))
        }
    }

    /// Returns true if the buffer contains no pending rows.
    pub fn is_empty(&self) -> bool {
        match self {
            BuffRepr::NonNullary(b) => b.is_empty(),
            BuffRepr::Nullary(b) => b.is_empty(),
        }
    }

    fn move_to_table(&mut self) -> TableRepr {
        match self {
            BuffRepr::NonNullary(b) => TableRepr::NonNullary(b.move_to_table()),
            BuffRepr::Nullary(b) => TableRepr::Nullary(b.move_to_table()),
        }
    }
}

/// A buffer of rows, with no guarantees on order or redundancy.
pub(crate) struct TableBuff<T> {
    /// Number of columns in each row.
    num_columns: usize,
    /// Flattened vector of rows.
    data: Vec<T>,
}

impl<T> TableBuff<T> {
    pub fn new(num_columns: usize) -> Self {
        assert!(num_columns > 0);
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
        assert!(num_columns > 0);
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
                "Table with no columns in not supported (due to unnatural flattened representation, use NullaryTable instead)"
            ),
            1 => Table::new(Self::into_chunks::<1>(data)),
            2 => Table::new(Self::into_chunks::<2>(data)),
            3 => Table::new(Self::into_chunks::<3>(data)),
            4 => Table::new(Self::into_chunks::<4>(data)),
            n => {
                let mut rows: Vec<Box<[Sym]>> = data.chunks(n).map(Box::from).collect();
                rows.sort_unstable();
                rows.dedup();
                Table {
                    num_columns: n,
                    data: rows.into_iter().flat_map(|b| b.into_vec()).collect(),
                }
            }
        }
    }

    /// Returns true if the table is empty (has no rows)
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Extends this table with the rows of another table.
    fn merge(&mut self, recent: Table) {
        assert_eq!(self.num_columns, recent.num_columns);
        let data = std::mem::take(&mut self.data);
        let data = match self.num_columns {
            0 => unreachable!(),
            1 => crate::merge::merge_unique(Self::into_chunks::<1>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            2 => crate::merge::merge_unique(Self::into_chunks::<2>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            3 => crate::merge::merge_unique(Self::into_chunks::<3>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            4 => crate::merge::merge_unique(Self::into_chunks::<4>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            n => {
                let a_rows: Vec<Box<[Sym]>> = data.chunks(n).map(Box::from).collect();
                let b_rows: Vec<Box<[Sym]>> = recent.data.chunks(n).map(Box::from).collect();
                let merged = crate::merge::merge_unique(a_rows, b_rows);
                merged.into_iter().flat_map(|b| b.into_vec()).collect()
            }
        };
        self.data = data
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
            0 => unreachable!(),
            1 => self.retain_distinct_spec::<1>(reference),
            2 => self.retain_distinct_spec::<2>(reference),
            3 => self.retain_distinct_spec::<3>(reference),
            4 => self.retain_distinct_spec::<4>(reference),
            n => {
                let data = std::mem::take(&mut self.data);
                let mut data: Vec<Box<[Sym]>> = data.chunks(n).map(Box::from).collect();
                let mut reference = reference.data.chunks(n);
                let mut next_ref = reference.next();
                data.retain(|row| {
                    while let Some(r) = next_ref {
                        if r < row.as_ref() {
                            next_ref = reference.next();
                        } else {
                            break;
                        }
                    }
                    next_ref.is_none_or(|r| r != row.as_ref())
                });
                self.data = data.into_iter().flat_map(|b| b.into_vec()).collect();
            }
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
    pub(crate) stable: Rc<RefCell<TableRepr>>,
    /// Rows that were added in the previous iteration of the program.
    ///
    /// Rows are deduplicated and not present in `stable`
    pub(crate) recent: Rc<RefCell<TableRepr>>,
    /// Rows that have been produced in the current iteration.
    /// Potentially contains redundancies.
    pub(crate) to_add: Rc<RefCell<BuffRepr>>,
}

impl VarTable {
    pub(crate) fn new(num_columns: usize) -> Self {
        Self {
            stable: Rc::new(RefCell::new(TableRepr::new_empty(num_columns))),
            recent: Rc::new(RefCell::new(TableRepr::new_empty(num_columns))),
            to_add: Rc::new(RefCell::new(BuffRepr::new(num_columns))),
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
        match &*self.stable.borrow() {
            TableRepr::NonNullary(table) => table.num_columns,
            TableRepr::Nullary(_) => 0,
        }
    }

    /// Adds a row to the table.
    pub fn add(&self, row: impl AsRef<[Sym]>) {
        let row = row.as_ref();
        match &mut *self.to_add.borrow_mut() {
            BuffRepr::NonNullary(b) => b.push(row),
            BuffRepr::Nullary(b) => {
                assert!(row.is_empty(), "nullary predicate received a non-empty row");
                b.push();
            }
        }
    }

    /// Adds several rows into the table.
    pub fn extend<'a>(&self, rows: impl IntoIterator<Item = &'a [Sym]>) {
        let mut buff = self.to_add.borrow_mut();
        match &mut *buff {
            BuffRepr::NonNullary(b) => b.extend(rows),
            BuffRepr::Nullary(b) => {
                for row in rows {
                    assert!(row.is_empty(), "nullary predicate received a non-empty row");
                    b.push();
                }
            }
        }
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

    /// If nullary, returns true if table contains a row, and false otherwise.
    /// If non-nullary, returns the table with all elements in the relation.
    ///
    /// Panics if the variable has unprocessed elements.
    pub fn extract<'me>(&'me self) -> VarTableExtract<'me> {
        assert!(
            self.stable(),
            "VarTable has unprocessed elements, the program likely did not run to completion."
        );

        match &*self.stable.borrow() {
            TableRepr::NonNullary(_) => {
                VarTableExtract::NonNullary(Ref::map(self.stable.borrow(), |repr| match repr {
                    TableRepr::NonNullary(table) => table,
                    TableRepr::Nullary(_) => unreachable!(),
                }))
            }
            TableRepr::Nullary(nullary_table) => VarTableExtract::<'_>::Nullary(nullary_table.has_row()),
        }
    }

    pub(crate) fn process(&self) {
        // move recent to stable
        let mut recent = TableRepr::new_empty(self.arity());
        std::mem::swap(&mut recent, &mut self.recent.borrow_mut());
        self.stable.borrow_mut().merge(recent);

        // create table of the elements to add (sorted with no duplicates)
        let mut new = self.to_add.borrow_mut().move_to_table();
        // remove from new elements all those already in stable
        new.retain_distinct(&self.stable.borrow());
        // copy those in the recent set (erasing all the recent one that were move to stable)
        *self.recent.borrow_mut() = new;
    }
}

/// Result of [`VarTable::extract`]. Carries either a borrow on the non-nullary predicate's
/// table or the truth value of a nullary predicate.
pub enum VarTableExtract<'me> {
    /// View over a non-nullary predicate.
    NonNullary(Ref<'me, Table>),
    /// Truth value of a nullary predicate.
    Nullary(bool),
}

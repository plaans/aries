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
    /// Flattened vector of rows (only meaningful when `num_columns > 0`)
    data: Vec<T>,
    /// Whether the table contains the unit row (only meaningful when `num_columns == 0`).
    /// This is necessary because, when a table has no columns it can not represent its rows in a flattened vector.
    ///
    /// This boolean encodes the two possible states of the table:
    ///
    /// 1) it is empty  (`has_unit_row == false`)
    /// 2) it contains exactly one "row", the unit row []   (`has_unit_row == true`)
    has_unit_row: bool,
}

impl<T> TableBuff<T> {
    pub fn new(num_columns: usize) -> Self {
        Self {
            num_columns,
            data: Default::default(),
            has_unit_row: false,
        }
    }

    pub fn push(&mut self, row: &[T])
    where
        T: Clone,
    {
        assert_eq!(row.len(), self.num_columns);
        self.data.extend_from_slice(row);
        if self.num_columns == 0 {
            // we are adding something to the table, which is necessarily the unit row
            self.has_unit_row = true
        }
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
        if self.num_columns == 0 {
            // we have no columns so the flattened representation is always empty
            // we have either 0 or 1 unit row ([]) which is captured by the `has_unit` entry
            itertools::Either::Left(std::iter::repeat_n([].as_slice(), self.has_unit_row as usize))
        } else {
            itertools::Either::Right(self.data.chunks(self.num_columns))
        }
    }

    pub fn is_empty(&self) -> bool {
        // empty if we either have no data in in the flat representation (num_columns >= 1)
        // or no the unit element (num_columns == 0)
        debug_assert!(!self.has_unit_row || self.num_columns == 0);
        self.data.is_empty() && !self.has_unit_row
    }
}

impl TableBuff<Sym> {
    pub fn move_to_table(&mut self) -> Table {
        if self.num_columns != 0 {
            // extract all rows from data and replace it with an empty vec
            let data = std::mem::take(&mut self.data);
            // move the the rows into a new `Table`
            Table::new_from_flat(self.num_columns, data)
        } else {
            // create a new table that contain the unit row if we have it
            let mut table = Table::new_empty(0);
            table.has_unit = self.has_unit_row;
            // remove unit row in ourself
            self.has_unit_row = false;
            table
        }
    }
}

/// A table with rows of fixed size, that are ordered and unique.
pub struct Table {
    /// Number of elements in each row
    num_columns: usize,
    /// This is a flattened representation of `Vec<[Sym; num_columns]>`
    ///
    /// Note that this is only meaningful when `num_columns > 0`
    data: Vec<Sym>,
    /// Whether the table contains the unit row (only meaningful when `num_columns == 0`).
    /// This is necessary because, when a table has no columns it can not represent its rows in a flattened vector.
    ///
    /// This boolean encodes the two possible states of the table:
    ///
    /// 1) it is empty  (`has_unit_row == false`)
    /// 2) it contains exactly one "row", the unit row []   (`has_unit_row == true`)
    has_unit: bool,
}

impl Table {
    /// Creates a new empty table with the given number of columns.
    pub fn new_empty(num_columns: usize) -> Self {
        Self {
            num_columns,
            data: Vec::new(),
            has_unit: false,
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
            has_unit: false,
        }
    }

    /// Creates a new table from a flattened vector of rows.
    pub(crate) fn new_from_flat(num_columns: usize, data: Vec<Sym>) -> Table {
        match num_columns {
            0 => panic!("Table with no columns cannot be built from flattened representation"),
            1 => Table::new(Self::into_chunks::<1>(data)),
            2 => Table::new(Self::into_chunks::<2>(data)),
            3 => Table::new(Self::into_chunks::<3>(data)),
            4 => Table::new(Self::into_chunks::<4>(data)),
            5 => Table::new(Self::into_chunks::<5>(data)),
            n => {
                let mut rows = into_chunks_generic(data, n);
                rows.sort_unstable();
                rows.dedup();
                Table {
                    num_columns: n,
                    data: into_flattened_generic(rows),
                    has_unit: false,
                }
            }
        }
    }

    /// Number of columns in the table.
    pub fn num_columns(&self) -> usize {
        self.num_columns
    }

    /// Returns true if the table is empty (has no rows)
    pub fn is_empty(&self) -> bool {
        debug_assert!(!self.has_unit || self.num_columns == 0);
        self.data.is_empty() && !self.has_unit
    }

    /// Extends this table with the rows of another table.
    fn merge(&mut self, recent: Table) {
        assert_eq!(self.num_columns, recent.num_columns);
        let data = std::mem::take(&mut self.data);
        let data = match self.num_columns {
            0 => {
                // no columns, the table may only contain the unit element
                // the resulting table contains the unit element it either one does
                self.has_unit |= recent.has_unit;
                return;
            }
            1 => crate::merge::merge_unique(Self::into_chunks::<1>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            2 => crate::merge::merge_unique(Self::into_chunks::<2>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            3 => crate::merge::merge_unique(Self::into_chunks::<3>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            4 => crate::merge::merge_unique(Self::into_chunks::<4>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            5 => crate::merge::merge_unique(Self::into_chunks::<5>(data), Self::into_chunks(recent.data))
                .into_flattened(),
            n => {
                let a_rows = into_chunks_generic(data, n);
                let b_rows = into_chunks_generic(recent.data, n);
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
        const { assert!(N != 0) }
        assert_eq!(self.num_columns, N);
        self.data.as_chunks().0
    }

    /// Returns an iterator over all rows in the table.
    ///
    /// See [`Self::rows_sized`] when the number of columns is know at compile time.
    pub fn rows(&self) -> impl Iterator<Item = &[Sym]> {
        if self.num_columns == 0 {
            itertools::Either::Left(std::iter::repeat_n([].as_slice(), self.has_unit as usize))
        } else {
            itertools::Either::Right(self.data.chunks(self.num_columns))
        }
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
            0 => {
                // if reference has unit row, we can't have it as well
                self.has_unit &= !reference.has_unit;
            }
            1 => self.retain_distinct_spec::<1>(reference),
            2 => self.retain_distinct_spec::<2>(reference),
            3 => self.retain_distinct_spec::<3>(reference),
            4 => self.retain_distinct_spec::<4>(reference),
            5 => self.retain_distinct_spec::<5>(reference),
            n => {
                let data = std::mem::take(&mut self.data);
                let mut data = into_chunks_generic(data, n);
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
        const { assert!(N != 0) }
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

fn into_chunks_generic(data: Vec<Sym>, n: usize) -> Vec<Box<[Sym]>> {
    data.chunks(n).map(Box::from).collect()
}
fn into_flattened_generic(data: Vec<Box<[Sym]>>) -> Vec<Sym> {
    data.into_iter().flat_map(|b| b.into_vec()).collect()
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
        let mut recent = Table::new_empty(self.recent.borrow().num_columns);
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

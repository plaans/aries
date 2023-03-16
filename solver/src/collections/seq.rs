use std::collections::HashSet;
use std::hash::Hash;

/// A trait implemented by most collection of elements, the objective being to
/// allow quite broad pattern on the calling side.
///
/// ```
/// use aries::collections::seq::Seq;
/// fn print(items: impl Seq<u32>) {
///     println!("{:?}", items.to_vec());
/// }
/// print([1, 3, 4]);
/// print(vec![3, 4, 5]);
/// ```
pub trait Seq<T> {
    fn to_vec(self) -> Vec<T>;
    fn to_set(self) -> HashSet<T>
    where
        T: Hash + Eq;
}

impl<Collection, T> Seq<T> for Collection
where
    Collection: IntoIterator<Item = T>,
{
    fn to_vec(self) -> Vec<T> {
        self.into_iter().collect()
    }

    fn to_set(self) -> HashSet<T>
    where
        T: Hash + Eq,
    {
        self.into_iter().collect()
    }
}

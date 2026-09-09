/// Assuming `entries` is sorted w.r.t. the given comparison function
/// (i.e. it is a sequence of `Less`, then `Equal`, then `Greater` elements),
/// returns the `[start, end)` index range of the `Equal` elements, or `None` if there are none.
pub fn binary_search_range_by<T, F: FnMut(&T) -> Ordering>(entries: &[T], mut cmp: F) -> Option<(usize, usize)> {
    let start = entries.partition_point(|elt| cmp(elt) == Ordering::Less);
    // `entries[start]`, if any, is `Equal` or `Greater`, so this is the length of the `Equal` run.
    let len = entries[start..].partition_point(|elt| cmp(elt) == Ordering::Equal);

    (len != 0).then_some((start, start + len))
}

use std::cmp::Ordering;

/// Co-iterates `a` and `b`, yielding the pairs of chunks (maximal runs) sharing a same key.
/// Keys present in only one of the two slices are skipped over.
///
/// Both slices must be sorted by `key` !
///
/// Skips and runs are traversed by galloping (see [`gallop_run_end`]).
pub fn merge_join_chunks_by_key<'a, T, K: Ord>(
    a: &'a [T],
    b: &'a [T],
    key: impl Fn(&T) -> K,
) -> impl Iterator<Item = (&'a [T], &'a [T])> {
    let (mut i, mut j) = (0usize, 0usize);

    std::iter::from_fn(move || {
        while i < a.len() && j < b.len() {
            let ka = key(&a[i]);
            let kb = key(&b[j]);

            match ka.cmp(&kb) {
                // Jump straight to the first element that could match, not just past this run.
                Ordering::Less => i = gallop_run_end(a, i, |x| key(x) < kb),
                Ordering::Greater => j = gallop_run_end(b, j, |x| key(x) < ka),
                Ordering::Equal => {
                    let i_end = gallop_run_end(a, i, |x| key(x) == ka);
                    let j_end = gallop_run_end(b, j, |x| key(x) == kb);
                    let chunks = (&a[i..i_end], &b[j..j_end]);
                    (i, j) = (i_end, j_end);
                    return Some(chunks);
                }
            }
        }
        None
    })
}

/// Index of the first element after `start` for which `pred` is false, or `s.len()`.
/// Assumes `pred` is "true-then-false" over `s[start..]`, and true at `start`.
///
/// Galloping: an exponential probe followed by a binary search bounded by the bracket it finds.
/// Costs a single comparison when the run has length 1, and `O(log d)` where `d` is the distance to the answer.
/// Unlike `partition_point`, which is `O(log n)` in the length of the whole remaining slice regardless of how near the answer is.
#[inline]
fn gallop_run_end<T>(s: &[T], start: usize, pred: impl Fn(&T) -> bool) -> usize {
    debug_assert!(start < s.len() && pred(&s[start]));

    let mut lo = start;
    let mut step = 1;
    while start + step < s.len() && pred(&s[start + step]) {
        lo = start + step;
        step *= 2;
    }
    // `pred` holds at `lo`, and fails at `hi` unless `hi == s.len()`.
    let hi = (start + step).min(s.len());
    lo + 1 + s[lo + 1..hi].partition_point(&pred)
}

/// Assuming `a` and `b` are sorted w.r.t the given comparison function,
/// merges `a` into `b`, replacing `b` and draining `a`.
/// *Consecutive* duplicate elements are ignored (i.e. exactly equal ones, not those 'equivalent' according to the comparison function).
/// But !! WARNING !! this means that if the comparison function is not a total order,
/// and the entries were sorted with an unstable sorted,
/// then some duplicate elements could end up not one after the other, and thus could be left in !!!
pub fn merge_dedup_into<T: PartialEq>(a: &mut Vec<T>, b: &mut Vec<T>, cmp: impl Fn(&T, &T) -> Ordering) {
    if a.is_empty() {
        return;
    }

    if b.is_empty() {
        std::mem::swap(b, a);
        return;
    }

    // Save the capacity before draining `a`.
    let capacity = a.len() + b.len();

    let old_b = std::mem::take(b);
    let mut a_iter = a.drain(..);
    let mut b_iter = old_b.into_iter();

    let mut result = Vec::with_capacity(capacity);

    while let (Some(a_ref), Some(b_ref)) = (a_iter.as_slice().first(), b_iter.as_slice().first()) {
        match cmp(a_ref, b_ref) {
            Ordering::Less => {
                result.push(a_iter.next().unwrap());
            }
            Ordering::Greater => {
                result.push(b_iter.next().unwrap());
            }
            Ordering::Equal => {
                let a_elem = a_iter.next().unwrap();
                let b_elem = b_iter.next().unwrap();

                if a_elem == b_elem {
                    result.push(a_elem);
                } else {
                    result.push(a_elem);
                    result.push(b_elem);
                }
            }
        }
    }

    result.extend(a_iter);
    result.extend(b_iter);

    *b = result;
}

#[cfg(test)]
mod tests {
    use crate::ext::lprelax::encoding::groundings::utils::{
        binary_search_range_by, merge_dedup_into, merge_join_chunks_by_key,
    };

    #[test]
    fn test_binary_search_range_by() {
        let entries = vec![(5, 3), (5, 6), (6, 3), (6, 5), (6, 8), (12, 10), (13, 8)];
        assert!(entries.is_sorted());

        assert_eq!(binary_search_range_by(&entries, |elt| elt.0.cmp(&6)), Some((2, 5)));
        assert_eq!(
            binary_search_range_by(&entries, |elt| if elt.0 < 6 {
                std::cmp::Ordering::Less
            } else if elt.0 == 6 && elt.1 < 7 {
                std::cmp::Ordering::Equal
            } else {
                std::cmp::Ordering::Greater
            }),
            Some((2, 4))
        );
        assert_eq!(binary_search_range_by(&entries, |elt| elt.0.cmp(&5)), Some((0, 2)));
        assert_eq!(binary_search_range_by(&entries, |elt| elt.cmp(&(6, 5))), Some((3, 4)));

        assert_eq!(binary_search_range_by(&entries, |elt| elt.0.cmp(&13)), Some((6, 7)));

        assert_eq!(binary_search_range_by(&entries, |elt| elt.0.cmp(&1)), None);
        assert_eq!(binary_search_range_by(&entries, |elt| elt.0.cmp(&7)), None);
        assert_eq!(binary_search_range_by(&entries, |elt| elt.0.cmp(&20)), None);

        assert_eq!(binary_search_range_by(&[] as &[(i32, i32)], |elt| elt.0.cmp(&6)), None);
    }

    #[test]
    fn equal_sort_keys_are_both_retained() {
        type Entry = (i32, i32, i32, i32);

        let cmp = |a: &Entry, b: &Entry| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then_with(|| a.2.cmp(&b.2));

        let mut a = vec![(1, 10, 20, 100), (2, 10, 20, 200)];

        let mut b = vec![(1, 10, 20, 101), (2, 10, 10, 100), (2, 10, 20, 200), (3, 10, 20, 300)];

        merge_dedup_into(&mut a, &mut b, cmp);

        assert!(a.is_empty());
        assert!({
            let cand1 = vec![
                (1, 10, 20, 101),
                (1, 10, 20, 100),
                (2, 10, 10, 100),
                (2, 10, 20, 200),
                (3, 10, 20, 300),
            ];
            let cand2 = vec![
                (1, 10, 20, 100),
                (1, 10, 20, 101),
                (2, 10, 10, 100),
                (2, 10, 20, 200),
                (3, 10, 20, 300),
            ];
            b == cand1 || b == cand2
        });
    }

    #[test]
    fn remaining_nonconsecutive_duplicates_with_partial_order() {
        type Entry = (i32, i32, i32, i32);
        let cmp = |a: &Entry, b: &Entry| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then_with(|| a.2.cmp(&b.2));

        let mut a = vec![(1, 1, 1, 1), (1, 1, 1, 2), (1, 1, 1, 3), (1, 1, 1, 4)];
        let mut b = vec![(1, 1, 1, 0), (1, 1, 1, 1), (1, 1, 1, 2), (1, 1, 1, 3)];

        merge_dedup_into(&mut a, &mut b, cmp);

        assert_eq!(
            b,
            vec![
                (1, 1, 1, 1),
                (1, 1, 1, 0),
                (1, 1, 1, 2),
                (1, 1, 1, 1), // dup of index 0
                (1, 1, 1, 3),
                (1, 1, 1, 2), // dup of index 2
                (1, 1, 1, 4),
                (1, 1, 1, 3), // dup of index 4
            ]
        );
    }

    #[test]
    fn test_merge_join_chunks_by_key() {
        let a = [(0, 'a'), (0, 'b'), (2, 'c'), (3, 'd'), (3, 'e')];
        let b = [(0, 'x'), (1, 'y'), (3, 'z')];

        let got: Vec<_> = merge_join_chunks_by_key(&a, &b, |&(k, _)| k).collect();
        assert_eq!(got, vec![(&a[0..2], &b[0..1]), (&a[3..5], &b[2..3])]);

        // disjoint keys, and empty inputs
        assert_eq!(merge_join_chunks_by_key(&a, &[(1, 'y')], |&(k, _)| k).count(), 0);
        assert_eq!(merge_join_chunks_by_key(&a, &[], |&(k, _)| k).count(), 0);
        assert_eq!(merge_join_chunks_by_key(&[], &b, |&(k, _)| k).count(), 0);
    }
}

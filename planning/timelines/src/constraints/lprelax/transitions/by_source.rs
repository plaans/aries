use crate::analysis::Source;

/// An entry for each source: one for the empty source, and one for each (included) concrete source.
#[derive(Clone, Debug, Default)]
pub(crate) struct BySource<T> {
    empty_source: T,
    concrete_sources: idmap::DirectIdMap<crate::TaskId, T>,
}
impl<T: Default> BySource<T> {
    pub fn get(&self, source: Source) -> Option<&T> {
        match source {
            None => Some(&self.empty_source),
            Some(task_id) => self.concrete_sources.get(task_id),
        }
    }

    pub fn get_mut_or_default(&mut self, source: Source) -> &mut T {
        match source {
            None => &mut self.empty_source,
            Some(task_id) => {
                if !self.concrete_sources.contains_key(task_id) {
                    self.concrete_sources.insert(task_id, T::default());
                }
                self.concrete_sources.get_mut(task_id).unwrap()
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Source, &T)> {
        std::iter::chain(
            [(None, &self.empty_source)],
            self.concrete_sources
                .iter()
                .map(|(task_id, entry)| (Some(task_id), entry)),
        )
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> {
        std::iter::chain(
            [&mut self.empty_source],
            self.concrete_sources.iter_mut().map(|(_, entry)| entry),
        )
    }
}



/// Implementors of this trait can export a snapshot of their runtime statistics.
/// The exported data must be able to _at least_ be formatted using std::fmt::Display.
/// Ideally, it should also implement serde traits when `export_stats` feature is enabled.
pub trait SnapshotStatistics {
    type Stats: Sized + std::fmt::Display;

    fn snapshot_statistics(&self) -> Self::Stats;
}

// SPDX-License-Identifier: GPL-3.0-or-later
//! The beacon view's state (QGDXC-TUI-0102); the schedule and report matching are
//! the library's (Annex G).
pub use qso_graph_dxcluster::beacons::*;
use std::time::Duration;

/// A report of a beacon, kept for one beacon on one band (QGDXC-TUI-0102).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heard {
    pub at: Duration,
    /// The source it came from, an index into the model's sources.
    pub source: usize,
    pub snr_db: Option<i64>,
    pub spotter: String,
}

/// Reports younger than this count as recent.
pub const RECENT: Duration = Duration::from_secs(15 * 60);

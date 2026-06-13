//! A simple traversal budget: caps files examined to keep scans fast (§10).

use std::cell::Cell;

/// Tracks how many files a traversal has examined against a hard cap.
pub struct FsBudget {
    max_files: usize,
    examined: Cell<usize>,
}

impl FsBudget {
    pub fn new(max_files: usize) -> FsBudget {
        FsBudget {
            max_files,
            examined: Cell::new(0),
        }
    }

    /// Record one examined file; returns `false` once the cap is exceeded.
    pub fn tick(&self) -> bool {
        let n = self.examined.get() + 1;
        self.examined.set(n);
        n <= self.max_files
    }

    pub fn exhausted(&self) -> bool {
        self.examined.get() >= self.max_files
    }

    pub fn examined(&self) -> usize {
        self.examined.get()
    }
}

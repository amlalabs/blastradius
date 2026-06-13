//! Tiny stderr progress spinner for slow phases (the probe battery, transcript
//! ingestion, retro scoring). Purely cosmetic and always on stderr, so it never
//! contaminates the value-free report on stdout.
//!
//! When stderr is a terminal it animates a one-line spinner in place; otherwise
//! (piped, CI, headless) it prints a single start line. Either way it prints a
//! `✓` done line with elapsed time, so a long scan never looks hung.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Run `f` while showing a progress indicator labelled `label` on stderr.
/// Returns whatever `f` returns. Prints an elapsed-time done line when finished.
pub fn spin<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let tty = std::io::stderr().is_terminal();
    let done = Arc::new(AtomicBool::new(false));

    let handle = if tty {
        let done = Arc::clone(&done);
        let label = label.to_string();
        Some(thread::spawn(move || {
            let mut i = 0usize;
            while !done.load(Ordering::Relaxed) {
                // \r returns to column 0; trailing spaces clear any prior longer line.
                eprint!("\r  {} {}…        ", FRAMES[i % FRAMES.len()], label);
                let _ = std::io::stderr().flush();
                i += 1;
                thread::sleep(Duration::from_millis(90));
            }
        }))
    } else {
        eprintln!("  ▸ {label}…");
        None
    };

    let out = f();

    done.store(true, Ordering::Relaxed);
    if let Some(h) = handle {
        let _ = h.join();
        eprint!("\r\x1b[2K"); // clear the spinner line
        let _ = std::io::stderr().flush();
    }
    eprintln!("  ✓ {label} ({:.1}s)", start.elapsed().as_secs_f32());
    out
}

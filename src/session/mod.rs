//! Session blast-radius scoring (§23) + AUTO-SLURP transcript ingestion and
//! retro-hazard detection (§24).
//!
//! This is an **additive runtime overlay** on the static scanner — it adds no
//! probes and no new detection regex. It joins observed agent `AgentEvent`s
//! against the real §11–§12 `Finding`s a live `scan` produces, scores the
//! session, names toxic-combination security paths, and (for retro) re-resolves
//! historical sessions against the current baseline.
//!
//! Value-free discipline (§23.11 / §24.4) is load-bearing: nothing past the
//! Layer-0 extractor / Layer-1 `normalize.rs` boundary ever carries a secret
//! value. See the per-module docs for the layering.
//!
//! SCAFFOLD STATUS: the type backbone (the frozen contracts) is complete and
//! compiles; logic-bearing functions return empty/default values until the
//! follow-up phases fill them in.

pub mod classify;
pub mod discovery;
pub mod history;
pub mod normalize;
pub mod report;
pub mod retro;
pub mod score;
pub mod toxic_combinations;
pub mod trace;

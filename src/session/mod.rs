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
//! STATUS: the engine is fully implemented and wired into the `sessions`,
//! `audit-history`, and `dashboard` subcommands (not a scaffold).

pub mod classify;
pub mod conformance;
pub mod discovery;
pub mod history;
pub mod normalize;
pub mod report;
pub mod retro;
pub mod score;
pub mod toxic_combinations;
pub mod trace;

/// Pinned identity of the compiled-in session rule pack (Seam C, beacon's
/// `spec/threat-rules/VERSION` analog). Bumped only on a deliberate corpus
/// change so any rule add/remove/rename forces an explicit version decision.
///
/// **Frozen-contract note:** this const is intentionally **NOT** surfaced into
/// [`report::SessionReport`] or `report/json.rs` — the JSON `schema_version`
/// (`1.0`) is a frozen, byte-identical contract and adding a field would break
/// `determinism_byte_identical`. `RULE_PACK_VERSION` is an internal/test-facing
/// identity only.
pub const RULE_PACK_VERSION: &str = "session-rules/v1";

#[cfg(test)]
mod mod_tests {
    use super::RULE_PACK_VERSION;

    #[test]
    fn rule_pack_version_is_pinned() {
        assert_eq!(RULE_PACK_VERSION, "session-rules/v1");
    }
}

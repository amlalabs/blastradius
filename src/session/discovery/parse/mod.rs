//! §24.2.1 — one extractor per distinct transcript line shape. MVP implements
//! JsonlClaude (claude-code/factory/devin block model) and JsonlCodex (a
//! DISTINCT value-free extractor — its function_call/event_msg bodies are
//! world-readable `0644` and secret-bearing, NOT the Claude block model).
//!
//! Every other shape is a detect-only stub in MVP (`DetectedUnparsed`).

pub mod jsonl_beacon;
pub mod jsonl_claude;
pub mod jsonl_codex;

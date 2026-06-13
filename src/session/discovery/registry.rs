//! §24.1 — the discovery agent registry as an explicit data table so the
//! inventory of every directory opened stays auditable (mirrors
//! `probes/registry.rs`). MVP parses JsonlClaude (claude-code/factory) and
//! JsonlCodex; every other agent is DETECTED but `DetectedUnparsed(reason)`.

use crate::session::discovery::locate::Root;
use crate::session::discovery::AgentSpec;
use crate::session::retro::SourceKind;

/// The §24.1 agent registry. CLI agents are `$HOME`-relative and identical on
/// Linux/macOS; only IDE-backed agents diverge (macOS `MacAppSupport` vs Linux
/// `XdgConfig`/`XdgData`), so those probe both roots per source. Only the two
/// MVP parsers (JsonlClaude, JsonlCodex) have an empty `unparsed_reason`.
pub fn agents() -> &'static [AgentSpec] {
    AGENTS
}

static AGENTS: &[AgentSpec] = &[
    AgentSpec {
        agent_tag: "claude-code",
        roots: &[Root::Home],
        discovery_marker: ".claude/settings.json",
        transcript_glob: ".claude/projects/*/*.jsonl",
        source_kind: SourceKind::JsonlClaude,
        unparsed_reason: "",
    },
    AgentSpec {
        agent_tag: "codex",
        roots: &[Root::Home],
        discovery_marker: ".codex/config.toml",
        transcript_glob: ".codex/sessions/*/*/*/rollout-*.jsonl",
        source_kind: SourceKind::JsonlCodex,
        unparsed_reason: "",
    },
    AgentSpec {
        agent_tag: "factory",
        roots: &[Root::Home],
        discovery_marker: ".factory",
        transcript_glob: ".factory/sessions/*.jsonl",
        source_kind: SourceKind::JsonlClaude,
        unparsed_reason: "",
    },
    AgentSpec {
        agent_tag: "copilot",
        roots: &[Root::Home],
        discovery_marker: ".copilot/config.json",
        transcript_glob: ".copilot/session-state/*/events.jsonl",
        source_kind: SourceKind::JsonlCopilot,
        unparsed_reason: "JsonlCopilot parser not yet implemented",
    },
    AgentSpec {
        agent_tag: "cursor",
        roots: &[Root::Home],
        discovery_marker: ".cursor/hooks.json",
        transcript_glob: ".cursor/projects/*/agent-transcripts/*.jsonl",
        source_kind: SourceKind::JsonlCursor,
        unparsed_reason: "JsonlCursor parser not yet implemented",
    },
    AgentSpec {
        agent_tag: "cursor-ide",
        roots: &[Root::MacAppSupport, Root::XdgConfig],
        discovery_marker: "Cursor/User",
        transcript_glob: "Cursor/User/**/state.vscdb",
        source_kind: SourceKind::SqliteVscdb,
        unparsed_reason: "sqlite feature off",
    },
    AgentSpec {
        agent_tag: "opencode",
        roots: &[Root::XdgData, Root::XdgConfig],
        discovery_marker: "opencode",
        transcript_glob: "opencode/storage/message/*/msg_*.json",
        source_kind: SourceKind::JsonDir,
        unparsed_reason: "JsonDir parser not yet implemented",
    },
    AgentSpec {
        agent_tag: "gemini",
        roots: &[Root::Home],
        discovery_marker: ".gemini/settings.json",
        transcript_glob: ".gemini/tmp/*/chats/*",
        source_kind: SourceKind::JsonGemini,
        unparsed_reason: "JsonGemini parser not yet implemented",
    },
    AgentSpec {
        agent_tag: "antigravity",
        roots: &[Root::Home],
        discovery_marker: ".gemini/config/hooks.json",
        transcript_glob: ".gemini/antigravity-cli/brain/*/.system_generated/logs/transcript.jsonl",
        source_kind: SourceKind::JsonlAntigravity,
        unparsed_reason: "JsonlAntigravity parser not yet implemented",
    },
    AgentSpec {
        agent_tag: "devin",
        roots: &[Root::XdgConfig, Root::Home],
        discovery_marker: ".codeium/windsurf/hooks.json",
        transcript_glob: "",
        source_kind: SourceKind::JsonlClaude,
        unparsed_reason: "no fixed glob; hook-supplied path only",
    },
    AgentSpec {
        agent_tag: "windsurf",
        roots: &[Root::Home],
        discovery_marker: ".codeium/windsurf/hooks.json",
        transcript_glob: ".codeium/windsurf/**/state.vscdb",
        source_kind: SourceKind::SqliteVscdb,
        unparsed_reason: "sqlite feature off",
    },
    AgentSpec {
        agent_tag: "aider",
        roots: &[Root::CurrentRepo, Root::Home],
        discovery_marker: ".aider.chat.history.md",
        transcript_glob: ".aider.chat.history.md",
        source_kind: SourceKind::MarkdownAider,
        unparsed_reason: "MarkdownAider parser not yet implemented",
    },
    AgentSpec {
        agent_tag: "hermes",
        roots: &[Root::Home],
        discovery_marker: ".hermes/config.yaml",
        transcript_glob: ".hermes/state.db",
        source_kind: SourceKind::SqliteVscdb,
        unparsed_reason: "sqlite (deferred); detect-only",
    },
    AgentSpec {
        agent_tag: "amp",
        roots: &[Root::XdgConfig],
        discovery_marker: "amp/settings.json",
        transcript_glob: "",
        source_kind: SourceKind::JsonDir,
        unparsed_reason: "undocumented / cloud-synced; detect-only",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_mvp_parsers_and_honest_unparsed() {
        let claude = agents()
            .iter()
            .find(|a| a.agent_tag == "claude-code")
            .unwrap();
        assert!(claude.unparsed_reason.is_empty());

        let codex = agents().iter().find(|a| a.agent_tag == "codex").unwrap();
        assert!(codex.unparsed_reason.is_empty());

        // Everything else is honestly detect-only in MVP.
        let cursor = agents().iter().find(|a| a.agent_tag == "cursor").unwrap();
        assert!(!cursor.unparsed_reason.is_empty());
    }
}

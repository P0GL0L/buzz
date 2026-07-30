# ASV Agent and Skill Fabric

## Goal

Let Buzz coordinate every approved ASV agent and make the team's skills
discoverable without pretending that Buzz merges private memory, credentials,
filesystems, or host capabilities.

Buzz is the signed conversation and routing layer. Each agent runtime remains
the execution authority for its own host, identity, tools, memory, approvals,
and credentials.

## Current observed shape

The current MacBook Buzz nest lists Bumble, Codex, Fizz, Honey, Jarvis, and
Vision as active managed agents. Earlier verified private-lane evidence also
included Claude and Charles. Cody on POGsAlien is a requested participant but
does not yet have a locally observed Buzz identity in this checkout.

Those are inventory facts, not live readiness claims. Before adding or
dispatching any identity, recheck:

1. The current relay profile and public key.
2. The host that owns the runtime.
3. Channel membership.
4. Mention and inbound-sender policy.
5. Authentication/credential readiness.
6. A real request and signed reply.

Never reuse one Nostr keypair for two profiles.

## Collaboration room

Keep `Command Group v2` small for Charles, Vision, and Jarvis. Use a separate
private `agent-lab` room for the first whole-team collaboration pilot, or the
existing `agent-operations` lane if Charles prefers not to create another
room.

Pilot roster:

- Charles — approval authority
- Vision — lead/orchestrator on the MacBook
- Jarvis — Mac mini operations and infrastructure
- Cody — POGsAlien specialist after identity and host custody are verified
- Bumble, Codex, Fizz, Honey, and Claude — focused specialists only when
  explicitly mentioned

Operating policy:

- Mention-gated dispatch.
- One lead agent per task.
- One active turn per agent at first.
- A lead asks a named specialist a bounded question.
- Specialists return findings, evidence, or a blocker; they do not recursively
  summon the room.
- Single-writer repository ownership unless the task names disjoint paths.
- Charles retains approval, external-send, deployment, and spend authority.
- Stop a thread after the lead posts a synthesis or requests human direction.

## Skill registry model

Do not bulk-copy every app's skill directory into every agent home. That would
mix platform-specific instructions, create stale duplicates, and risk moving
connection or credential assumptions across trust boundaries.

Instead, Buzz should maintain a provenance-bearing capability index:

- Skill ID and display name
- Source host and runtime
- Source package or app
- Entrypoint location
- Content hash and observed timestamp
- Supported artifact or task types
- Required connectors and permissions
- Availability state: available, unavailable, degraded, or unknown
- Invocation owner: the agent/host that can actually run it
- Sharing policy: private, team-indexed, or portable

The registry schema for this fork lives in
`docs/asv-local/skill-registry.schema.json`.

Agents receive a small router skill that:

1. Searches the Buzz registry.
2. Selects an eligible host/agent.
3. Posts a bounded handoff with the source skill ID and task evidence.
4. Waits for the owning agent's signed result.
5. Returns the result to the original thread with provenance.

Portable skills may later be packaged and synchronized after license,
dependency, and secret scanning. Host-bound skills remain remotely invocable
through their owning agent.

## Product milestones

### A. Inventory and truth

- Inventory Codex, Claude, Hermes, app-plugin, and local skill roots on the
  MacBook.
- Inventory Jarvis on the Mac mini through the existing Hermes/Buzz gateway.
- Inventory Cody on POGsAlien through its real host connection.
- Deduplicate by source ID plus content hash.
- Publish only metadata to Buzz.

### B. Private agent lab

- Resolve a distinct Buzz identity for every pilot participant.
- Create or select the private room.
- Apply mention gating and inbound allowlists.
- Prove Charles-to-agent, Vision-to-agent, and agent-to-lead replies.
- Record offline/degraded identities without substituting local stand-ins.

### C. Skill routing

- Add relay events for capability registry heads and observations.
- Add `buzz-cli skills list`, `skills show`, and `skills route`.
- Add desktop registry/search UI.
- Add a router prompt to managed-agent base context.
- Preserve per-host authorization at invocation time.

### D. Portable skill synchronization

- Add provenance, license, dependency, and secret scans.
- Add owner-reviewed publication.
- Add version pinning and rollback.
- Prove the same portable skill on two hosts before calling it shared.


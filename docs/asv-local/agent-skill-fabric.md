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

Keep existing operational channels separate. The first pilot runs in private
`agent-lab` channel `8e683b8f-14d6-4543-84cb-0a2c44ba00f4`.

Pilot roster:

- Charles — approval authority
- Vision — lead/orchestrator on the MacBook
- Jarvis — Mac mini operations and infrastructure
- Cody — POGsAlien specialist after identity and host custody are verified;
  currently excluded
- Bumble, Codex, Fizz, Honey, and Claude — later focused specialists only after
  separate admission

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

The complete host-custodied schema lives in
`docs/asv-local/skill-registry.schema.json`. The only shape permitted on a
relay is `docs/asv-local/skill-registry-relay-safe.schema.json`; it omits local
paths, exact hosts, connector details, permission evidence, content hashes,
and verification evidence.

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
- Publish only the signed relay-safe projection to Buzz.

The CLI treats discovery and execution readiness as separate facts:

- `catalogued` — present in an optional or upstream catalogue only.
- `installed` — present in a runtime-owned installed root, but not proven
  callable.
- `callable` — observed in a root that the named runtime currently exposes.
- `degraded` — owned and installed, but a required runtime dependency is
  failing.
- `unreachable` — last-known inventory whose owning host cannot be verified.

Only a non-expired `callable` observation is routable. Expired observations
become `unknown`; they never silently remain available.

Example local scan:

```bash
buzz skills scan \
  --host vision-macbook \
  --runtime codex \
  --owner <VISION_AGENT_PUBKEY> \
  --source callable:team-indexed:codex:$HOME/.codex/skills \
  --source catalogued:private:hermes:$HOME/.hermes
```

The complete registry and relay-safe projection default to
`~/.buzz-dev/skill-registry/`. Use `buzz skills validate`, `skills list`, and
`skills show` locally before any publication.

### B. Private agent lab

- Resolve a distinct Buzz identity for every pilot participant.
- Create or select the private room.
- Apply mention gating and inbound allowlists.
- Prove Charles-to-agent, Vision-to-agent, and agent-to-lead replies.
- Record offline/degraded identities without substituting local stand-ins.

### C. Skill routing

- Publish signed, searchable relay-safe registry chunks into the private room
  with `buzz skills publish`. Publication validates a hard forbidden-field
  denylist before sending.
- Use `buzz skills list`, `skills show`, and `skills route`.
- Add desktop registry/search UI.
- Add a router prompt to managed-agent base context.
- Preserve per-host authorization at invocation time.

The first signed projections were published on 2026-07-30 by their actual
owners: Vision published 248 MacBook records and rotated Jarvis published 277
Mac mini records. All are currently installed or catalogued rather than
callable, so the searchable shared index intentionally has zero routable
records. Literal documentation `@names` are enclosed in fenced registry JSON
and do not trigger message mention resolution.

`buzz skills route` selects only a current routable observation, verifies the
owning agent identity is a Nostr pubkey and current channel member, then emits
a bounded `skill-route/v1` request. The request carries a UUID correlation ID,
registry digest/revision, expected owner, skill ID, task bounds, and required
evidence. It never contains the canonical entrypoint or host evidence.

### D. Portable skill synchronization

- Add provenance, license, dependency, and secret scans.
- Add owner-reviewed publication.
- Add version pinning and rollback.
- Prove the same portable skill on two hosts before calling it shared.

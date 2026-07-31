# Trusted Agent-Lab Runbook

## Proof lanes

Keep these lanes separate in every report:

1. **Local development:** Docker Compose dependencies and the host relay at
   `ws://localhost:3000`.
2. **Desktop native:** the isolated `xyz.block.buzz.app.dev` bundle using
   `buzz-desktop-dev.main`.
3. **Hosted collaboration:** `wss://asv-labs.communities.buzz.xyz`.
4. **Human acceptance:** named on-device review of the Tauri application.

The standalone development identity must never connect to the hosted relay.
The local relay remains bound to loopback. Mobile pairing may advertise a
private, TLS-terminated tailnet front door, but must never use a public Funnel
or widen the relay listener beyond loopback.

## Development identity

Use only:

```bash
just desktop-native-qa-preflight
just fresh=1 desktop-standalone
```

`desktop-standalone` disables identity import, uses the scoped development
keyring, and starts no relay or database. Packaged debug applications embed the
validated scope because a Finder launch does not inherit the build shell's
environment; a valid runtime scope still takes precedence for `tauri dev` and
test instances. Capture a snapshot, quit, relaunch without `fresh=1`, capture a
second snapshot, then compare:

```bash
scripts/desktop-native-qa.sh snapshot restart-1
scripts/desktop-native-qa.sh snapshot restart-2
scripts/desktop-native-qa.sh compare restart-1 restart-2
```

Set the relay-local profile display name to `ASV Buzz Dev` only after the local
relay is healthy. Do not publish that profile to the hosted relay.

## Local stack

Docker Desktop is the selected macOS engine. Installation and first-launch
license acceptance remain separate gates; a human must accept Docker's terms.
After first launch, enable **Start Docker Desktop when you sign in** and disable
**Open Docker Dashboard when Docker Desktop starts**. Closing the dashboard
window does not stop the engine. Do not use **Quit Docker Desktop** while ASV
Buzz is connected to the local relay.

```bash
just setup
just local-stack-verify
just relay
just local-stack-verify --with-relay
```

`just relay` and `just dev` start an installed Docker Desktop through its
background CLI when the engine is stopped, then wait for it before starting
the Compose services. They do not use `open -a Docker` or require the dashboard
window to remain visible.

The Compose project owns persistent Postgres, MinIO, and Prometheus volumes.
All published development-service ports bind to `127.0.0.1`; do not widen
those bindings for tailnet access.
`docker compose down` stops services and preserves those volumes.
`just local-stack-verify --with-relay` also requires an accepted WebSocket
upgrade, not only HTTP health responses.

Before destructive schema testing:

```bash
just local-stack-backup
just local-stack-restart-proof
```

The backup command creates a mode-private logical Postgres backup, migration
ledger, image inventory, and hashes under the ignored `build/` tree. A reset is
allowed only through the repository's confirmed development-only reset command:

```bash
just reset
```

Never add `--volumes`, delete named volumes, or restore a database unless the
operator explicitly selects the exact backup and destructive test scope.

## Security stop gate

During isolation verification on 2026-07-29, the installed production signing
identity was found copied into legacy and scoped development keyrings, and its
private credential was exposed to the local task transcript during diagnosis.
Both development copies were removed; the production keyring and production
application data were left untouched. Standalone imports are now disabled and
a new isolated identity passed two-restart persistence.

Treat the old production signer as compromised. Charles authorized production
identity rotation and the resulting hosted membership migration on 2026-07-30.
The replacement independently proved relay and channel-owner authority before
the private pilot was created. Provider-account ownership transfer remains
separate: do not represent the old relay owner as revoked, delete the protected
rollback record, or claim Builderlab transfer until the provider confirms the
replacement account owns the hosted community.

## Hosted pilot admission order

1. Rotate Charles if authorized; prove the installed app signs with the new
   identity and revoke the old relay/channel memberships.
2. Rotate Jarvis on the Mac mini, with no private key in process arguments,
   service definitions, or logs.
3. Create private `agent-lab` with exactly Charles, Vision, and rotated Jarvis.
4. Apply inbound allowlists, mention gating, one active turn, one lead, no
   recursive summons, and single-writer repository ownership.
5. Prove every requested direction and an unlisted-sender rejection.
6. Admit no Cody identity until POGsAlien custody and a signed challenge/reply
   are independently verified.

Existing hosted channels and memberships remain out of scope except for the
explicitly approved rotations and the new room.

For Jarvis, disable the old desktop-managed runtime before restarting
production Buzz. Add the replacement role-for-role, read every new membership
back, then remove and permanently restrict the old key. A private key must
enter the process only through protected host storage and a wrapper:

```bash
launchctl print gui/$(id -u)/com.asvlabs.buzz-jarvis-agent-lab
```

Use `docs/asv-local/com.asvlabs.buzz-jarvis-agent-lab.plist.example` as the
non-secret service template. Keep the hosted relay URL in the launchd
environment so Hermes tool subprocesses inherit it. Do not leave debug logging
enabled after proof.

## Skill publication

Validate both representations on the owning host. Publish the relay-safe file
with the owning agent key, never the host-custodied canonical registry:

```bash
buzz skills validate ~/.buzz-dev/skill-registry/canonical.json
buzz skills validate --relay-safe ~/.buzz-dev/skill-registry/relay-safe.json
buzz skills list --registry ~/.buzz-dev/skill-registry/relay-safe.json --routable
buzz skills publish \
  --registry ~/.buzz-dev/skill-registry/relay-safe.json \
  --channel 8e683b8f-14d6-4543-84cb-0a2c44ba00f4
```

Published JSON is fenced to prevent literal documentation `@names` from being
treated as message mentions. Installed, catalogued, degraded, unreachable, or
expired records remain searchable but not routable. A routing failure must
name the unavailable owner/state; it must never select a stand-in.

## Rollback

- Desktop code: revert the focused development-isolation, workspace, or skill
  registry commit independently.
- Development app state: use `just fresh=1 desktop-standalone`; it targets only
  development bundle/keyring identifiers.
- Containers: `docker compose down` preserves volumes.
- Hosted pilot: remove the new room memberships and revoke newly admitted
  identities. For a role-preserving identity rotation, restore the old role
  only from the protected rollback record and only after lifting its explicit
  relay restriction.
- Skills: stop publishing the registry revision, retain the prior signed digest,
  and rescan the owning host. Never substitute a local stand-in.

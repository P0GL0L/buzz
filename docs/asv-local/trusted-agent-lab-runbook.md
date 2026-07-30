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
The local relay must not be bound or advertised to a tailnet during this
milestone.

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

```bash
just setup
just local-stack-verify
just relay
just local-stack-verify --with-relay
```

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

Treat the production signer as compromised. Do not create `agent-lab`, rotate
Jarvis memberships, or publish registry records to the hosted relay until
Charles explicitly authorizes production identity rotation and the resulting
membership migration.

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

## Rollback

- Desktop code: revert the focused development-isolation, workspace, or skill
  registry commit independently.
- Development app state: use `just fresh=1 desktop-standalone`; it targets only
  development bundle/keyring identifiers.
- Containers: `docker compose down` preserves volumes.
- Hosted pilot: remove the new room memberships and revoke newly admitted
  identities; do not mutate pre-existing rooms.
- Skills: stop publishing the registry revision, retain the prior signed digest,
  and rescan the owning host. Never substitute a local stand-in.

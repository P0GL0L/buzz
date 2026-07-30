# Agent-Lab Foundation Evidence — 2026-07-29

This is an implementation snapshot, not a claim that the hosted pilot is live.
Local, native, hosted, and human-acceptance gates remain separate.

## Fork and identity isolation

- Branch: `codex/asv-workspace-foundation`
- Reader/browser checkpoint: `13d945cf`
- Local/native hardening checkpoint: `63c855f0`
- Skill-fabric checkpoint: `e7319758`
- Bundle: `xyz.block.buzz.app.dev`
- Keyring service: `buzz-desktop-dev.main`
- Isolated development public key:
  `522ef9e0628458da21256548d376360c53bf508584eb50d33cb9c5d7cd56658b`
- Two-restart comparison:
  `developmentIdentityPersistent=true`,
  `productionStateUnchanged=true`
- Packaged debug application is identified by macOS as `Buzz Dev`.
- A stale production-key autofill attempted an identity import during native
  onboarding and was rejected by the standalone import guard.

## Security incident and stop gate

The installed production signer was present in both legacy and scoped
development keyrings. A diagnostic command exposed that private credential to
the local task transcript. Development copies were removed and a new
development-only key was generated. The production keyring and application
data were not changed.

The production signer must be rotated before it is used to administer a new
hosted room. That rotation is not implied by the original plan and requires
Charles's explicit approval.

## Native and web QA

- Desktop unit suite: 3,786 passing tests.
- Native Tauri suite: 1,878 passing, 14 ignored OS-keychain/real-relay tests,
  plus three passing mixer diagnostics. The default full-suite concurrency
  reproduced process-probe flakes, including while other pre-push suites ran
  concurrently. The repository recipe now serializes this native suite; the
  complete suite passed under the bounded setting.
- Focused reader/browser/video Playwright matrix: 10 passing tests.
- Workspace matrix covers Markdown, PDF, malformed PDF, DOCX, XLSX, PPTX,
  unknown files, images, browser navigation, blocked-frame fallback, restart,
  and keyboard close.
- Packaged native screenshots were captured with secrets masked.
- Packaged `Buzz Dev` exposed labelled controls through macOS accessibility.
  Keyboard traversal moved in order from `Join a community` to
  `Create a community` to `I already have a community`, and reverse traversal
  returned to `Create a community`. Return activated the focused control.
- The ASV application-design verifier does not support macOS/Tauri.
- Named human visual acceptance remains pending; the agent-run keyboard and
  accessibility inspection is not represented as human acceptance.

Repository gate status:

- Workspace Rust formatting and warning-denied clippy passed.
- Desktop checks, native formatting/clippy/check/tests, desktop build, web
  checks, and web build passed.
- Unit targets passed: `buzz-core` 232, `buzz-auth` 45, `buzz-db` 94 with
  151 database-dependent tests ignored, `buzz-conformance` 22, and
  `buzz-push-gateway` 15 with six Postgres-dependent tests ignored.
- `just ci` is not represented as fully green. It reached the unchanged mobile
  gate, where Hermit required a new 2,145,610,346-byte Flutter 3.41.7 SDK
  download before `dart format`, `flutter analyze`, or `flutter test` could
  start. The direct and checksum-verified remote-assisted fetches were stopped
  after proving the link was bandwidth-bound; no mobile result was inferred.

## Docker and local stack

- Docker Desktop ARM64 4.84.0 was downloaded from Docker's official endpoint.
- The application passed macOS code-signature and Gatekeeper validation.
- Charles completed Docker's first-launch configuration, password
  authorization, and license acceptance.
- Docker Engine 29.6.2 is running with the `overlayfs` storage driver.
- The task runner now locates Docker Desktop's bundled CLI on macOS, so the
  documented `just` commands do not depend on an optional global symlink.
- `just setup` completed against the persistent development Compose project.
- Postgres, Redis, Keycloak, MinIO, Prometheus, and Adminer recovered healthy.
  Keycloak uses its supported bootstrap variables and the management-port
  readiness endpoint.
- Every published development-service port is restricted to `127.0.0.1`.
- All 26 migrations applied successfully; the seeded local-only community has
  four loopback aliases and no tailnet address.
- `just local-stack-verify --with-relay` passed dependency health, migration
  state `26:26`, relay health/readiness, and an HTTP `101` WebSocket upgrade.
- `just local-stack-restart-proof` passed after adding a bounded health wait.
  A full Docker Desktop stop/start also preserved the same migration state,
  community count, named volumes, and relay readiness after restart.
- A post-restart logical backup was written at
  `build/local-stack-backups/20260730T080226Z` with directory mode `0700`;
  its database and migration-ledger checksums verified.
- Docker's ignored `NetworkType` experiment was removed before the recovery
  proof. The engine still reports `overlayfs`.
- The packaged desktop has not yet joined the local relay because the Mac
  locked before that native step. Local desktop connection and the
  `ASV Buzz Dev` relay-local display name remain pending.

## Jarvis rotation

- Old Jarvis public key:
  `e56246d57529c7545b9c07b13122a53a2bbe2ddb9e14407871f5d1aff183f820`
- Previously configured channel:
  `47c61e67-010c-4839-96f3-371188c45f8a`
- The old bridge exposed its private key in process arguments and accepted
  `respond-to=anyone`.
- The exact bridge was stopped and is no longer connected to the hosted relay.
- New Jarvis public key:
  `d98b269848d6aaa6eb31df0473e846cc4d9c65b38d6fde60cc34ff6f3d6462b2`
- The replacement secret is mode `0600` on the Mac mini and the wrapper is mode
  `0700`. Test process arguments and logs contained no private key.
- Offline signed challenge:
  `96af8488-4915-4cf9-bda0-6c5d4a4ee149`
- Verified Nostr event:
  `7773ad2fdb2aa433a62ad118e7fa29779a275e85cb925e872548d287b0f94e36`
- The new bridge remains offline. Old hosted membership revocation, new
  membership, and a hosted signed reply are pending the production-admin gate.

## Skill inventories

MacBook inventory:

- Canonical skills: 359
- Relay-safe records: 248
- Routable: 0
- Canonical and relay-safe validation passed.
- Registry files are mode `0600`.

Mac mini inventory:

- Canonical skills: 328
- Relay-safe records: 277
- Routable: 0
- Canonical and relay-safe validation passed on the owning host.
- Registry files are mode `0600`.

All current observations are intentionally `installed`, not `callable`.
Therefore no record is routable and nothing has been published to a relay.
POGsAlien/Cody remains excluded.

## Hosted status

- No `agent-lab` room was created.
- No registry record was published.
- No new identity was admitted to a hosted channel.
- Existing hosted channels were not intentionally changed.
- The old Jarvis bridge was stopped as an explicit security rotation action.

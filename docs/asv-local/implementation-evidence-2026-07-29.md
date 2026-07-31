# Agent-Lab Foundation Evidence — 2026-07-29 (updated 2026-07-30)

This is an implementation snapshot. The private hosted pilot is live, while
provider-account ownership transfer and named-human native visual acceptance
remain separate external gates. Local, native, hosted, and human-acceptance
proof are not interchangeable.

## Fork and identity isolation

- Branch: `codex/asv-workspace-foundation`
- Reader/browser checkpoint: `4193fb04`
- Local/native hardening checkpoint: `554dd615`
- Skill-fabric checkpoint: `214276f4`
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
- The first post-onboarding direct app relaunch exposed a scope-loss bug:
  Finder-launched debug bundles do not inherit `BUZZ_DEV_KEYRING_SERVICE` and
  fell back to the unscoped `buzz-desktop-dev` service. The packaged debug
  build now embeds the validated scoped service, while a valid runtime override
  remains higher priority.
- The accidental unscoped development key created by that failed launch was
  deleted. The scoped development and production keyring records remain
  present.
- After rebuilding, two consecutive direct application quits and relaunches
  reopened `Local Dev` as `ASV Buzz Dev` with no identity onboarding.

## Security incident and stop gate

The installed production signer was present in both legacy and scoped
development keyrings. A diagnostic command exposed that private credential to
the local task transcript. Development copies were removed and a new
development-only key was generated. The production keyring and application
data were not changed.

Charles explicitly authorized production identity rotation and the associated
hosted membership migration on 2026-07-30. Production Buzz now uses replacement
public key
`f9be6e874158486901ba431c53d295feaf2734a104bf80828214c785534ddab2`.
The exact pre-rotation keyring blob is retained as a protected rollback record
under a separate service. Two production-app restarts preserved the
replacement identity. The old relay owner remains available only for the
provider-account ownership gate; it is not represented as revoked.

## Native and web QA

- Desktop unit suite: 3,787 passing tests.
- Native Tauri suite: 1,881 passing, 14 ignored OS-keychain/real-relay tests,
  plus three passing mixer diagnostics. The default full-suite concurrency
  reproduced process-probe flakes, including while other pre-push suites ran
  concurrently. The repository recipe now serializes this native suite; the
  complete suite passed under the bounded setting.
- Focused reader/browser/video Playwright matrix: 10 passing tests.
- Workspace matrix covers Markdown, PDF, malformed PDF, DOCX, XLSX, PPTX,
  unknown files, images, browser navigation, blocked-frame fallback, restart,
  and keyboard close.
- The Office matrix now exercises rendered semantic DOCX/XLSX/PPTX states
  rather than the earlier unsupported placeholder. Six focused Rust tests
  cover headings/lists/tables/links, shared strings/sparse cells/formulas,
  ordered slides/notes, MIME mismatch, compound/encrypted containers,
  macro-bearing packages, invalid ZIP signatures, and Quick Look active-content
  sanitization.
- The dedicated workspace Playwright lane passes four scenarios on a
  configurable isolated test port. Port 4173 was already occupied by an
  unrelated local site, so the test runner was updated to accept
  `BUZZ_PLAYWRIGHT_PORT` without stopping that process.
- Live Quick Look rendering in the packaged macOS window remains part of the
  native visual gate; sanitizer unit coverage and browser-mock semantic
  screenshots are not represented as that native proof.
- The ADI-011 builder draft computes to 4.45/5.0 with no observed hard-fail,
  but its verdict remains FAIL because the design verifier cannot declare a
  macOS/Tauri target and independent review plus named-human acceptance are
  pending. The stored verifier receipt must be read as an unsupported-platform
  gate result, not a product-quality failure or an automated pass.
- Packaged native screenshots were captured with secrets masked.
- Packaged `Buzz Dev` exposed labelled controls through macOS accessibility.
  Keyboard traversal moved in order from `Join a community` to
  `Create a community` to `I already have a community`, and reverse traversal
  returned to `Create a community`. Return activated the focused control.
- The ASV application-design verifier does not support macOS/Tauri.
- Named human visual acceptance remains pending; the agent-run keyboard and
  accessibility inspection is not represented as human acceptance.

Native workspace browser:

- The right-side workspace now hosts a Tauri child webview with an isolated
  persistent Buzz Dev profile and an iframe fallback.
- Navigation is limited to HTTP(S). History, title, loading state, downloads,
  popups, page extraction, click/type/scroll actions, and clear controls are
  represented by versioned native commands and events.
- Browser downloads are restricted to the app-owned download directory and can
  be opened immediately by the artifact reader. Local preview reads prove the
  canonical path remains beneath that directory and enforce format-specific
  byte caps.
- The action stream redacts credentials, fragments, and sensitive query keys.
  It records the active controller without recording page input text.
- Native screenshot requests currently return a structured `unsupported`
  result. No success is claimed until a safe implementation is present.
- Focused Tauri tests, clippy with warnings denied, TypeScript typecheck, and
  the four-case workspace Playwright gate passed. Real-window browser layout
  and named-human acceptance remain separate gates.

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
- The packaged desktop joined only `http://localhost:3000`, created the
  relay-local profile `ASV Buzz Dev`, and opened the local private `Welcome`
  channel. The seeded welcome roster reached four local members.
- Two packaged-app restart cycles preserved the scoped identity, local
  community, profile name, and channel connection.

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
  `0700`. The launchd service invokes only the wrapper path, reads the key from
  protected storage, and exposes no private key in its arguments, plist, or
  logs.
- The service is limited to `agent-lab`, one active worker, mention dispatch,
  Charles as owner, and a Vision allowlist. Its isolated Hermes home is
  `~/.hermes/profiles/buzz-jarvis`.
- Hosted HTTP writes that are definitively rejected before the relay by
  Cloudflare Access now fall back to authenticated NIP-01 WebSocket publication
  of the exact already-signed event. Ambiguous delivery failures do not retry.
- Offline signed challenge:
  `96af8488-4915-4cf9-bda0-6c5d4a4ee149`
- Verified Nostr event:
  `7773ad2fdb2aa433a62ad118e7fa29779a275e85cb925e872548d287b0f94e36`
- Charles-to-Jarvis request
  `11d1c493783fca1d1b0984766910cc7f63c0543d2dd44b5f3e3429869f67bc0e`
  received the signed, correctly threaded runtime reply
  `3bc42999a1a6fc0550dc8e04c23be68fde837f26d1c8c75be395233f9e8d8fc4`.
- Vision-to-Jarvis request
  `53525859602239c74751caf7df10d4015000b9572f326305acc65a8ae94c2f35`
  received the signed, correctly threaded runtime reply
  `ab4c535e03285fec3401d95ab116eb809e5097b185817edbaee3d667e9e3cb31`.
- The replacement inherited the old Jarvis role in nine pre-existing channels
  only after every new membership was read back. The old key now has zero
  channel memberships.
- The old production-managed Jarvis entry has start and restart disabled. A
  production-app restart started Vision but did not restart old Jarvis.
- Permanent moderation event
  `aa03927df1f32199c7bac88830e7a085f99682f402f3488cd9c1cccd186a9892`
  revoked the old key. A fresh connection using that key was rejected with
  `blocked: you are banned from this community`.

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

All current observations are intentionally `installed` or `catalogued`, not
`callable`. Therefore no record is routable. A real route request against an
installed Vision record returned `no current routable observation`; it did not
fall back to another host or identity.

After relay-safe validation, Vision published its 248-record projection in
three signed events:

- `d152c4301c1071deb82487df6029570a40dd59291f631a37181e91c6528209ef`
- `4fad7311335af2c361b6ea51222313313656b8d6a543c21f46f768901ffaee5f`
- `d54ae993ad3898c161b659062b0749adf6b7a068687486c2716898465d6c2a9b`

Rotated Jarvis published its 277-record projection in three signed events:

- `347a11156066c46ba04cca81142830c18523f93b0f08a7354902028bd5e8f010`
- `6319241bbcb3ba0f3d9bebd96ee7a95121b016fec002be148b6c514848bc37a6`
- `2e4df0ff2b95fbf4a84129fa8e220cfe1356b829f114917b7e84554ba9c2f4ea`

Every event verified against its owning identity. Registry JSON is fenced so
literal skill documentation such as `@mention` is not interpreted as live
Buzz mentions. An earlier partial, unfenced publication was superseded after
the mention preflight safely rejected the affected chunks.

POGsAlien/Cody remains excluded.

## Buzz Dev workspace extensions

- The artifact workspace now classifies and opens bounded semantic DOCX, XLSX,
  and PPTX previews in addition to Markdown, text, images, and PDF.
- The native browser uses a Buzz Dev-scoped persistent webview profile, audited
  agent actions, HTTP(S)-only navigation, bounded page extraction, and
  artifact-routed downloads. Page capture remains an explicit unsupported
  result pending a safe native capture API.
- Settings → Agents now exposes non-secret connection records for Grok,
  Antigravity, and Gemini CLI with installed, pending-consent, connected,
  degraded, expired, and disconnected states.
- Provider runtime homes and allowlisted app-owned binaries resolve under the
  Buzz Dev application-data directory. Production Buzz and provider-global
  credential profiles are not migration sources.
- The bundled `buzz-antigravity-acp` bridge converts personal-plan Google
  requests into isolated `agy -p --output-format stream-json` tasks. It has no
  session-resume path, permits one active task, rejects non-text or oversized
  input, caps JSONL frames and output, enforces a wall-time limit, and kills
  the child process on cancellation.
- The adapter catalog entry remains unavailable until the official `agy`
  executable is present in the Buzz Dev provider directory. Runtime launch
  injects only the scoped provider home and executable path; it does not place
  OAuth material in arguments, agent definitions, logs, or relay records.
- Settings → Skills now renders the local relay-safe inventory with search and
  installation, availability, and runtime filters. The native reader caps and
  validates the projection, excludes canonical fields by construction, and
  demotes expired observations to non-routable `unknown`.
- Rust check and warnings-denied Clippy passed for the provider boundary.
  Desktop typecheck, file-size and text-scale checks passed. The focused
  provider settings Playwright gate passed on isolated port `4192`.
- Provider status reconciliation now separates installed, authenticated,
  runtime-verified, and hosted-admitted states. The desktop checks only for the
  presence of scoped provider-owned credential artifacts and never reads or
  projects their contents. A bounded no-tools verification action persists only
  a sanitized result and never reopens OAuth.
- Grok completed the bounded verification response and is displayed as
  connected with a verification timestamp. Google Antigravity is authenticated,
  but the provider returned an account-eligibility gate. Gemini CLI is
  authenticated, but the provider reports that the personal account route is
  unsupported and directs personal-plan use to Antigravity. Neither Google
  runtime is represented as callable or hosted-admitted.
- Provider reconciliation passed four focused Rust tests, warnings-as-errors
  Tauri Clippy, desktop typecheck, and the focused provider Playwright gate. The
  Developer ID-signed Buzz Dev bundle preserved `ASV Buzz Dev`, the existing
  channels, and all nine canonical agent definitions and portraits across two
  direct bundle restarts. Notarization remains a separate distribution gate.

## Hosted status

- Old Charles public key:
  `185c22c7a959c8d341fc4ab07216af3b083d0b9055ebe21a543d0bbbb17fd11c`
- Replacement Charles public key:
  `f9be6e874158486901ba431c53d295feaf2734a104bf80828214c785534ddab2`
- The replacement signer is the installed production identity. Its local
  signed challenge verified before any hosted write, and the production app
  retained it across restart.
- The old relay owner added the replacement as a relay administrator in event
  `830d2c7ba7391d849166745ff79442a0ffb937b3d11dfb83bb266491229bb850`.
  The relay-signed kind `13534` snapshot
  `61ca6821796e4c705c2bbccbd7d04ab09329904ea17657076da9fd19e897d3bb`
  independently reports exactly two direct members: the old owner and the new
  administrator.
- The replacement published its own hosted profile in event
  `d3861fc26c17b770546510d9ae37efab9c7d9950926a08fed4518b9fa95c1bbe`.
- The replacement is an owner in every pre-existing hosted channel visible to
  the production account. The old Charles identity remains relay owner and
  co-owner because the Builderlab provider account still reports no
  transferable community under the replacement login. This is an external
  provider-ownership gate, not a relay or channel-authority failure.
- Private channel `agent-lab`
  (`8e683b8f-14d6-4543-84cb-0a2c44ba00f4`) has exactly three members:
  replacement Charles as owner, Vision as bot, and rotated Jarvis as member.
- Charles-to-Vision request
  `1956a816d2090b1109b8bde4ad659d4096b2fdff6f9c87e5d45b5fb806a51f2c`
  received signed reply
  `7a3e95a9c35e4847d797480420505e0cd7928e52be7da3f22ddb53f4e5f203a2`.
- An unlisted test identity could neither authenticate to read the relay nor
  post a Jarvis mention. Jarvis logs contain no trace of the rejected token.
- Existing hosted content and channel settings were not changed. Membership
  changes were limited to the explicitly approved Charles/Jarvis rotations and
  the new private room.

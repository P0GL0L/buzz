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
- Focused reader/browser/video Playwright matrix: 10 passing tests.
- Workspace matrix covers Markdown, PDF, malformed PDF, DOCX, XLSX, PPTX,
  unknown files, images, browser navigation, blocked-frame fallback, restart,
  and keyboard close.
- Packaged native screenshots were captured with secrets masked.
- The ASV application-design verifier does not support macOS/Tauri.
- Named human keyboard, accessibility, and visual acceptance remains pending.

## Docker and local stack

- Docker Desktop ARM64 4.84.0 was downloaded from Docker's official endpoint.
- The application passed macOS code-signature and Gatekeeper validation.
- First-launch configuration, password authorization, and license acceptance
  remain a human action.
- Compose services, migrations, relay health, and restart recovery are not
  marked passed until Docker's engine is running.

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

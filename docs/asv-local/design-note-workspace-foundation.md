# ASV Buzz — Workspace Foundation Design Note

## Classification

Platform target(s): macOS desktop through Tauri 2; semantic document previews remain portable
App category & jobs-to-be-done: productivity; keep files, agent output, research, voice capture, and collaboration inside the active Buzz thread
Selected navigation model (+ per-platform expression): rail-split-listdetail; existing desktop rail and timeline with a contextual right-side workspace
Why this navigation fits the IA: the active conversation remains primary while one selected resource receives focused detail and controls
Selected screen composition (per key screen): message timeline plus contextual split-on-large-screens artifact or browser detail
Why this is not a generic template app (transplant test): removing signed Buzz messages, relay-backed attachments, agent provenance, and the persistent workspace would destroy the flow rather than leave a reusable dashboard shell
Platform conventions honored (HIG / Material / PWA), per platform: native macOS window, keyring and file dialogs, Quick Look fidelity, keyboard navigation, and system external-open actions
Style atlas match: existing Buzz structured, calm, dense-but-legible desktop grammar
Reference moves borrowed (max 3, with verification grade): existing Buzz right-panel grammar (local product reference); macOS Quick Look handoff (system behavior); no external screen copied
Template budget for this category / templated elements used / justification if > 0: one list/detail auxiliary workspace; within the productivity budget of one
Tab bar: no new tab bar
FAB: none
Onboarding: none for this feature
Forbidden defaults rejected (from the twelve tells): no reflexive five-tab bar, card wall, stat dashboard, blanket FAB, hidden primary navigation, onboarding carousel, identical-platform claim, wrapped-webview-as-native claim, happy-path-only states, modal-everything, unsafe chrome, or fixed type system
Store-asset/screenshot provenance: A Salty Vet cutout mark from the published first-party asset at `https://asaltyvet.com/media/brand/a-salty-vet-logo-cutout.png`; workspace capture at `desktop/test-results/native-qa/workspace-office-preview.png`
State design (loading, empty, error, offline, success) per key screen: explicit extraction or transcription loading, empty package or recording, structured/rendered/transcribed success, warning/truncation, malformed/encrypted/macro/oversized or microphone-denied error, offline state, and original-download or retained-audio recovery
Motion / gesture / haptics budget: existing short panel transition only; no theatrical delays or new haptics
Permissions strategy (just-in-time, primed, least-privilege, graceful-deny): same-relay media fetch only; no Office execution; sandboxed fidelity HTML; native open/download remain user actions; microphone permission is requested only when dictation starts and denial keeps typed composition available
Accessibility plan (screen reader, Dynamic Type / font scale, contrast, reduce motion, target size): labelled controls, semantic buttons and tables, keyboard close/navigation, inherited Buzz type and contrast tokens; named-human screen-reader review pending
Performance & battery risk: bounded compressed/expanded bytes, archive entries, XML parts, rows, cells, slides, assets, output text, and one blocking parser worker
Implementation stack (per platform): Tauri 2 Rust commands, React 19 workspace UI, macOS Quick Look optional fidelity, semantic OOXML fallback, bounded native audio capture, and a pluggable local/provider transcription boundary

- Product category: productivity / social collaboration
- Platform target: macOS desktop through the existing Tauri 2 application
- Build surface: ASV Buzz, the isolated Buzz development desktop
- Primary users: Charles and the private ASV agent team
- Primary job: keep research, files, agent output, and collaboration inside the active Buzz context
- Navigation model: existing desktop sidebar plus contextual right-side auxiliary workspace
- Back behavior: closing the auxiliary workspace returns focus to the invoking link or file card; browser history stays inside the workspace pane
- Store asset or screenshot provenance: first-party A Salty Vet cutout mark from the published website asset

Buzz Desktop is outside the current ADI verifier's iOS, Android, React Native,
and installable-PWA platform matrix. This note applies the ADI composition and
functional-truth constraints, but does not claim a machine-verified ADI pass.

## Product identity

The isolated desktop product is named **ASV Buzz**. The macOS display name,
bundle filename, app-switcher label, permission copy, and development-only icon
use that identity. The bundle identifier remains `xyz.block.buzz.app.dev`, the
keyring service remains `buzz-desktop-dev.main`, and the application-support
directory remains development-scoped. The rename must not migrate, copy,
replace, or reset identity data.

The app icon uses the first-party A Salty Vet cutout mark. It is generated into
a development-only icon directory referenced only by the development Tauri
configuration. The production Buzz icon set remains unchanged.

## Composition choices

### Message timeline

Keep the timeline as the primary surface. A file card remains inline evidence
and exposes two honest actions:

1. Open the resource in the right-side workspace.
2. Download the original through the existing native save flow.

The card must never claim a format is previewable merely because it is a file.

### Workspace reader

Use one contextual auxiliary panel with a stable header, resource metadata,
content area, download/open-external actions, loading state, error state, and
unsupported-format state.

Rendered formats:

- images
- PDF
- Markdown
- plain text and source text
- JSON
- CSV
- DOCX semantic structure: headings, paragraphs, lists, links, and tables
- XLSX worksheet tabs, bounded cells, inert formulas, filtering, and chart
  metadata
- PPTX ordered slides, titles, body text, and speaker notes

Office previews are dual-layered. The bounded semantic representation is the
portable baseline. On macOS, the reader may add a system Quick Look rendering
after stripping scripts, active elements, event handlers, external links, and
network-backed assets. The resulting HTML runs in a sandboxed frame with a
deny-by-default content policy. Legacy, encrypted, macro-bearing, malformed,
and oversized packages remain explicitly unsupported and downloadable.

### Browser

External links keep the existing OS-browser action and gain an explicit
`Open in Buzz browser` action. Phase one uses a sandboxed embedded frame with
manual address submission, a private local history stack, reload, and an
open-external fallback. Sites can refuse embedding; that refusal is an
expected state, not a successful browser claim.

The browser-control milestone now uses a Tauri child webview aligned to the
workspace content region. It has a Buzz Dev-only persistent data store,
bounded HTTP(S)-only navigation, observable history/loading/title state,
downloads routed back into the artifact reader, DOM/text extraction, selector
click/type, scrolling, immediate stop and clear controls, an active-controller
indicator, and a redacted local audit stream. The iframe remains the explicit
fallback when the native surface is unavailable. The milestone retains one
persistent session and defers tabs. On macOS, visible-region capture uses the
validated workspace rectangle and returns explicit permission, inactive-window,
occlusion, or capture failures; a failed request never fabricates an image.
Successful screenshots and downloads become immutable artifacts in the active
thread.

The workspace browser is always composed as a dedicated right-hand sibling of
the current ASV Buzz content surface. It must not use a backdrop, fixed-position
drawer, or any geometry that covers channel messages, a focused thread, agent
context, or settings content. Its left edge is resizable and the width is
clamped so the current context retains a usable column. Opening, navigating,
and closing the browser preserves the selected channel and thread underneath;
at narrow window widths the two columns compress within their explicit minimums
rather than changing into an overlapping sheet. The divider occupies a dedicated
gutter outside the native child webview and captures the active pointer so
webpage content cannot steal or strand the resize gesture. Pointer movement is
coalesced to one layout update per display frame. Native child-webview geometry
uses one observer and a latest-bounds-wins queue with at most one Tauri request
in flight, preventing resize events from outrunning macOS webview layout.

### Voice dictation

Dictation is a composer action, not a destination, tab, or autonomous agent
capability. The composer gains one microphone control and a composer-scoped
`Command-Shift-D` tap-toggle with five explicit states: idle, recording,
transcribing, cancelled, and recoverable error. A visible timer, input-level
indicator, stop/cancel controls, and active-agent/user ownership make capture
state unambiguous. The resulting transcript is inserted at the current editor
selection as an editable draft and is never sent automatically.

Audio capture is bounded to two minutes per session, 30 seconds of continuous
speech per recognition chunk, and the existing bounded PCM queue. Raw audio is
ephemeral and is never posted to the relay. This milestone uses the existing
local English Parakeet model only; provider transcription is deferred.
Permission denial, model missing/downloading, offline operation, timeout,
cancellation, empty speech, device loss, and a concurrent huddle microphone
are separate structured outcomes. Typed composition remains available in every
failure state.

### Artifact sessions and revisions

The workspace groups a message attachment, generated agent result, browser
download, screenshot, and each later revision into one thread-bound artifact
session. The original bytes are immutable. A revision is a new signed message
attachment with a stable artifact identifier, monotonically increasing
version, parent reference, SHA-256 digest, source class, signer, thread
reference, and optional task correlation ID. Message edits never overwrite
artifact bytes or silently rewrite revision history.

Revision history stays in the originating thread and the contextual workspace
for this milestone. There is no global artifact-library destination, new tab,
or separate card-wall home.

### Provider connections

Settings → Agents includes a provider surface for xAI Grok, Google
Antigravity, and Gemini CLI. It distinguishes installation, authentication
evidence, runtime verification, degraded, expired, and disconnected states.
Buzz checks only for the presence of provider-owned scoped credential artifacts;
it never reads their contents. Sign in launches the provider's official
interactive login in a visible terminal. Verify runs one bounded no-tools
provider response and persists only its sanitized pass/fail status. Disconnect
disables Buzz routing without reading or deleting provider-owned OAuth
credentials.

Provider installations, settings, and runtime homes remain under the current
Buzz Dev application-data directory. Runtime discovery may resolve only the
allowlisted `agy` and `gemini` executables from that tree. Grok and Gemini
agent processes receive app-scoped `GROK_HOME` and `GEMINI_CLI_HOME` values at
spawn time, and agent definitions cannot override those keys. The relay, agent
records, snapshots, audit logs, and provider status IPC contain no OAuth token.
An installed binary is never represented as authenticated without scoped
provider-owned credential evidence, and authentication is never represented as
runtime-verified without a successful bounded provider response.

Personal-plan Antigravity tasks use the bundled
`buzz-antigravity-acp` adapter. Each ACP prompt launches a new `agy -p` process
with `--output-format stream-json`; the adapter never uses `--continue` or
reuses a provider conversation. It accepts text-only bounded input, allows one
active task, bounds JSONL frames, stderr, final output, and wall time, and
terminates the provider process on ACP cancellation. Only the terminal
`result` event becomes an agent message. The official `agy` executable and its
provider-owned authentication state remain in the Buzz Dev provider scope.

### Skills catalog

Settings → Skills reads only the local relay-safe projection. Search and
filters cover capability, skill identity, owner identity, runtime class,
installation state, availability, and observation time. The desktop reader
revalidates the bounded schema and recomputes expiry on load; an expired
observation becomes `unknown` and cannot remain routable. Missing or malformed
inventories surface an explicit owning-host scan/validation recovery state.
The UI never opens the canonical registry and therefore cannot expose local
paths, host details, permission evidence, or connector configuration.

## Budgets and defaults

- No new global tab bar.
- No FAB.
- No onboarding carousel.
- No dashboard/card-wall home screen.
- One auxiliary workspace surface, opened contextually.
- Existing Buzz theme, type scale, icon family, and right-panel grammar remain
  authoritative.

## Twelve default-pattern rejections

1. No five-tab app shell.
2. No card-wall home.
3. No stat-tile dashboard.
4. No blanket FAB.
5. No onboarding pager.
6. No identical cross-platform shell claim.
7. No decorative fake system chrome.
8. No missing loading, empty, error, or unsupported states.
9. No raw color or type literals outside the existing token system.
10. No unsafe-area or window-chrome bypass.
11. No default-font/neon/uniform-card identity replacement.
12. No hidden truncation or capability-count claims.

## Functional truth

- “Preview” appears only for formats the phase-one renderer can actually open.
- Office packages are previewable only after their filename, MIME declaration,
  ZIP signature, archive budget, and required OOXML parts agree.
- Formulas and Office content are never executed. Macro-bearing and encrypted
  packages fail closed and remain downloadable.
- Browser embed failures direct the user to the existing external opener.
- The reader fetches relay media through the existing bounded native IPC path;
  it does not bypass Buzz's relay URL validation.
- Agent skill consolidation will index source skills with provenance and
  access policy. It will not copy credentials, silently merge private memory,
  or imply every host has the same tools.

## Acceptance targets for this slice

- A PDF file card opens the right-side reader and can still be downloaded.
- An image context menu can open the original in the right-side reader.
- A Markdown or text file renders readable content in the reader.
- DOCX, XLSX, and PPTX open bounded semantic previews with search, copy, zoom,
  and format-specific navigation.
- Legacy, macro-bearing, encrypted, malformed, or oversized Office files open
  an honest recoverable error and keep the original download action.
- A normal HTTP(S) message link can be opened in the Buzz browser from its
  context menu without removing the OS-browser option.
- Reader and browser states are keyboard-closeable and expose useful labels.
- The browser panel's left edge begins at or after the right edge of the active
  content pane, with no backdrop or overlap at every supported desktop width.
- Dragging the browser divider resizes only the two adjacent panes and
  double-clicking it restores the default browser width.
- Automated tests cover resource classification, URL normalization, file
  metadata formatting, and context-menu activation of the browser panel.

## Native QA evidence boundary

`just desktop-native-qa-preflight` must report the
`xyz.block.buzz.app.dev` bundle and the `buzz-desktop-dev.main` keyring service
before a main-checkout launch. `scripts/desktop-native-qa.sh snapshot LABEL`
records content hashes for the production support directory and nest file,
keyring metadata hashes, and only a one-way digest of the development identity.
It never prints or stores a secret.

Capture one snapshot before the first native launch and one after each of two
clean restarts. `scripts/desktop-native-qa.sh compare restart-1 restart-2` proves
that the development identity digest persisted and that production support,
production keyring metadata, and the production nest digest did not change.

The evidence lanes remain independent:

1. `just desktop-native-qa-web` — deterministic browser-mock state coverage
   and screenshots.
2. Tauri unit/smoke results — native bridge and platform behavior.
3. Native visual inspection — the real `Buzz Dev` window on macOS.
4. Human on-device acceptance — a named human verifies keyboard flow,
   accessibility labels, window layout, and the two-restart identity result.

The current ADI verifier has no macOS/Tauri platform fragment. This gate records
that limitation and must not convert it into an automated ADI pass.

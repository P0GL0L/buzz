# ASV Local Buzz — Workspace Foundation Design Note

## Classification

- Product category: productivity / social collaboration
- Platform target: macOS desktop through the existing Tauri 2 application
- Build surface: Buzz Desktop
- Primary users: Charles and the private ASV agent team
- Primary job: keep research, files, agent output, and collaboration inside the active Buzz context
- Navigation model: existing desktop sidebar plus contextual right-side auxiliary workspace
- Back behavior: closing the auxiliary workspace returns focus to the invoking link or file card; browser history stays inside the workspace pane
- Store asset or screenshot provenance: none

Buzz Desktop is outside the current ADI verifier's iOS, Android, React Native,
and installable-PWA platform matrix. This note applies the ADI composition and
functional-truth constraints, but does not claim a machine-verified ADI pass.

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

Phase-one rendered formats:

- PDF
- Markdown
- plain text and source text
- JSON
- CSV

DOCX, XLSX, PPTX, and legacy Office formats enter the same reader but show an
explicit conversion-not-yet-available state. This is an honest foundation for
the later native conversion/rendering layer.

### Browser

External links keep the existing OS-browser action and gain an explicit
`Open in Buzz browser` action. Phase one uses a sandboxed embedded frame with
manual address submission, a private local history stack, reload, and an
open-external fallback. Sites can refuse embedding; that refusal is an
expected state, not a successful browser claim.

The later browser-control milestone must use a native child webview or browser
automation bridge with observable navigation state and scoped agent authority.

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
- Office files are labeled as unsupported in this slice and remain
  downloadable.
- Browser embed failures direct the user to the existing external opener.
- The reader fetches relay media through the existing bounded native IPC path;
  it does not bypass Buzz's relay URL validation.
- Agent skill consolidation will index source skills with provenance and
  access policy. It will not copy credentials, silently merge private memory,
  or imply every host has the same tools.

## Acceptance targets for this slice

- A PDF file card opens the right-side reader and can still be downloaded.
- A Markdown or text file renders readable content in the reader.
- An unsupported Office file opens an honest fallback state.
- A normal HTTP(S) message link can be opened in the Buzz browser from its
  context menu without removing the OS-browser option.
- Reader and browser states are keyboard-closeable and expose useful labels.
- Automated tests cover resource classification, URL normalization, file
  metadata formatting, and context-menu activation of the browser panel.

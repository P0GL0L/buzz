# Buzz Desktop Native QA Scorecard

## Gate status

| Gate | Evidence command or artifact | Status rule |
| --- | --- | --- |
| Development isolation | `just desktop-native-qa-preflight` | Pass only for the dev bundle, dev keyring, and local relay |
| Production preservation | `desktop-native-qa.sh compare restart-1 restart-2` | Pass only when all production fingerprints are unchanged |
| Identity persistence | Same comparison | Pass only when a non-empty dev identity digest matches across two clean restarts |
| Workspace web matrix | `just desktop-native-qa-web` | Browser-mock evidence; never a native pass |
| Tauri smoke | `just desktop-tauri-test` | Native-code test result |
| Native visual | `build/native-qa/*` plus real-window screenshots | Manual evidence from `ASV Buzz` |
| Accessibility and keyboard | Named human checklist | Manual on-device acceptance |
| ADI verifier | `app-design-gate` brief/verifier | Unsupported for macOS/Tauri; recorded, not waived |

## On-device checklist

- Confirm the title and app switcher identify `ASV Buzz`.
- Confirm the Dock/Finder icon uses the A Salty Vet cutout mark and remains
  visually distinct from production Buzz.
- Confirm the window opens in the expected desktop layout without overlap or
  clipped controls.
- Confirm Tab reaches reader/browser controls in a sensible order.
- Confirm Escape closes the workspace and focus returns to the invoking item.
- Confirm accessibility labels for close, download, back, forward, reload,
  browser address, and external open.
- Exercise Markdown, PDF, an image, DOCX, XLSX, PPTX, malformed PDF, and an
  unknown file.
- Exercise embedded navigation, a site that blocks framing, external-open
  fallback, reload, app restart, and local-relay offline recovery.
- Record the reviewer name, timestamp, app commit, macOS version, pass/fail,
  and any defects below.

## Human acceptance record

- Reviewer: pending
- Timestamp: pending
- Commit: pending
- macOS: pending
- Result: pending
- Notes: pending

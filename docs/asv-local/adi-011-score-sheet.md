# ADI-011 Draft — ASV Buzz Collaboration Workspace

Builder self-review only. This draft is not an independent verdict and does not
replace named-human macOS acceptance.

Screen/flow: Composer dictation, artifact workspace, native browser, and Skills settings
Platform(s): macOS desktop through Tauri 2; semantic reader is cross-platform
A1 Navigation fit: 5/5 — evidence: `WorkspacePanel.tsx` keeps the message timeline primary and opens one contextual right-side list/detail surface; dictation remains a composer action and skill operations remain in Settings → Skills.
A2 Platform conformance: 4/5 — evidence: `office_preview/fidelity.rs` uses macOS Quick Look, `PdfArtifactReader.tsx` bundles PDF.js, and `workspace_browser.rs` uses the isolated Tauri child webview plus bounded macOS visible-region capture. Packaged-window visual review remains pending.
A3 State completeness: 5/5 — evidence: dictation exposes idle/recording/transcribing/cancelled/error; the readers preserve loading/ready/truncated/error/unsupported states; browser capture returns explicit inactive-window, permission/capture, invalid-bounds, oversized, and unsupported outcomes; skill routing reports non-routable states without substitution.
A4 Template justification: 5/5 — evidence: `design-note-workspace-foundation.md` declares one auxiliary workspace and rejects card walls, extra navigation, FABs, and onboarding.
A5 Visual identity system: 4/5 — evidence: dictation, PDF controls, browser audit state, artifact history, and Skills actions reuse ASV Buzz typography, semantic colors, icon family, border/radius grammar, and the established right-panel composition.
A6 Motion & haptics: 4/5 — evidence: `WorkspacePanel.tsx` uses the existing short panel transition with no theatrical loading delay; reduced-motion behavior remains inherited from the Buzz shell.
A7 Accessibility floor: 4/5 — evidence: reader and dictation controls carry accessible names, PDF navigation is keyboard-operable, Office modes and skill actions are native controls, document search/filter is labelled, and the panel remains keyboard-closeable. Named-human screen-reader review remains pending.
Weighted overall: 4.45/5.0
Hard-fails triggered: none observed in builder review
Verdict: FAIL — independent ADI-011 review, automated macOS/Tauri verification support, live Quick Look visual inspection, and named-human on-device acceptance remain pending.

Weights: A1 .20, A2 .20, A3 .15, A4 .10, A5 .15, A6 .10, A7 .10.

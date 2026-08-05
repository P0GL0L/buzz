# ASV Buzz workspace feature evidence

These screenshots are deterministic Playwright outputs from the workspace
acceptance cases. They demonstrate the tested rendered states; they do not
replace the separately pending named-human native visual, keyboard, and
accessibility acceptance gate.

## Bundled PDF.js reader

![Thread-bound PDF reader with thumbnail, navigation, zoom, search, copy, and download controls](screenshots/workspace-pdf-reader.png)

The PDF attachment remains visible in its originating thread while the bundled
reader opens in the right-side workspace.

## Persistent workspace browser

![Right-side workspace browser with navigation, controller state, fallback messaging, and recovery controls](screenshots/workspace-browser.png)

The browser preserves the channel/thread composition and presents its controller
state and structured fallback message inside the workspace.

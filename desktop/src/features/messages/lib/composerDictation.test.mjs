import assert from "node:assert/strict";
import test from "node:test";

import {
  formatDictationElapsed,
  shouldHandleDictationShortcut,
  transcriptInsertionText,
} from "./composerDictation.ts";

test("dictation shortcut is composer-scoped primary-shift-d", () => {
  assert.equal(
    shouldHandleDictationShortcut(
      {
        altKey: false,
        ctrlKey: false,
        key: "D",
        metaKey: true,
        shiftKey: true,
      },
      true,
    ),
    true,
  );
  assert.equal(
    shouldHandleDictationShortcut(
      {
        altKey: false,
        ctrlKey: false,
        key: "d",
        metaKey: true,
        shiftKey: false,
      },
      true,
    ),
    false,
  );
});

test("dictation transcript is normalized for editable insertion", () => {
  assert.equal(transcriptInsertionText("  hello   world  "), "hello world ");
  assert.equal(transcriptInsertionText("   "), "");
});

test("dictation timer is stable", () => {
  assert.equal(formatDictationElapsed(0), "0:00");
  assert.equal(formatDictationElapsed(61_400), "1:01");
});

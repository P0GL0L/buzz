import assert from "node:assert/strict";
import { test } from "node:test";

import {
  classifyArtifactPreview,
  formatWorkspaceFileSize,
  normalizeBrowserUrl,
} from "./workspaceResource.ts";

test("classifyArtifactPreview recognizes PDF by MIME or extension", () => {
  assert.equal(
    classifyArtifactPreview({
      filename: "report.bin",
      mime: "application/pdf",
    }),
    "pdf",
  );
  assert.equal(classifyArtifactPreview({ filename: "report.PDF" }), "pdf");
});

test("classifyArtifactPreview distinguishes markdown and readable text", () => {
  assert.equal(classifyArtifactPreview({ filename: "handoff.md" }), "markdown");
  assert.equal(
    classifyArtifactPreview({
      filename: "results.bin",
      mime: "application/json; charset=utf-8",
    }),
    "text",
  );
  assert.equal(classifyArtifactPreview({ filename: "ledger.csv" }), "text");
});

test("classifyArtifactPreview recognizes images by MIME or extension", () => {
  assert.equal(
    classifyArtifactPreview({
      filename: "capture.bin",
      mime: "image/png",
    }),
    "image",
  );
  assert.equal(classifyArtifactPreview({ filename: "capture.WEBP" }), "image");
});

test("classifyArtifactPreview recognizes supported OOXML Office files", () => {
  assert.equal(
    classifyArtifactPreview({
      filename: "brief.docx",
      mime: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    }),
    "docx",
  );
  assert.equal(
    classifyArtifactPreview({
      filename: "model.xlsx",
      mime: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    }),
    "xlsx",
  );
  assert.equal(
    classifyArtifactPreview({
      filename: "deck.pptx",
      mime: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    }),
    "pptx",
  );
  assert.equal(
    classifyArtifactPreview({ filename: "legacy.doc" }),
    "unsupported",
  );
});

test("normalizeBrowserUrl adds https and rejects unsafe schemes", () => {
  assert.equal(normalizeBrowserUrl("example.com"), "https://example.com/");
  assert.equal(
    normalizeBrowserUrl("https://example.com/docs?q=1"),
    "https://example.com/docs?q=1",
  );
  assert.equal(normalizeBrowserUrl("javascript:alert(1)"), null);
  assert.equal(normalizeBrowserUrl("file:///tmp/private"), null);
  assert.equal(normalizeBrowserUrl(""), null);
});

test("formatWorkspaceFileSize uses compact binary units", () => {
  assert.equal(formatWorkspaceFileSize(undefined), "");
  assert.equal(formatWorkspaceFileSize(820), "820 B");
  assert.equal(formatWorkspaceFileSize(12_700), "12 KB");
});

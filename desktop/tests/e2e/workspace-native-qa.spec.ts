import { expect, type Page, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";
import { onePagePdf } from "../helpers/pdf";

const SHA = "9".repeat(64);

async function waitForLiveSubscription(page: Page) {
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (
            window as Window & {
              __BUZZ_E2E_HAS_MOCK_LIVE_SUBSCRIPTION__?: (input: {
                channelName: string;
              }) => boolean;
            }
          ).__BUZZ_E2E_HAS_MOCK_LIVE_SUBSCRIPTION__?.({
            channelName: "general",
          }) ?? false,
      ),
    )
    .toBe(true);
}

function emitAttachment(
  page: Page,
  filename: string,
  mime: string,
  url: string,
  image = false,
) {
  return page.evaluate(
    ({ filename, image, mime, sha, url }) => {
      const emit = (
        window as Window & {
          __BUZZ_E2E_EMIT_MOCK_MESSAGE__?: (input: {
            channelName: string;
            content: string;
            extraTags: string[][];
          }) => void;
        }
      ).__BUZZ_E2E_EMIT_MOCK_MESSAGE__;
      if (!emit) throw new Error("Mock message emitter is unavailable.");
      emit({
        channelName: "general",
        content: image ? `![${filename}](${url})` : `[${filename}](${url})`,
        extraTags: [
          [
            "imeta",
            `url ${url}`,
            `m ${mime}`,
            `x ${sha}`,
            "size 128",
            ...(image ? ["dim 1x1"] : []),
            `filename ${filename}`,
          ],
        ],
      });
    },
    { filename, image, mime, sha: SHA, url },
  );
}

async function openFileCard(page: Page, filename: string) {
  const card = page
    .getByTestId("file-card")
    .filter({ hasText: filename })
    .last();
  await expect(card).toBeVisible();
  await card.getByTestId("file-card-preview").click();
  await expect(page.getByTestId("workspace-panel")).toBeVisible();
}

async function closeWorkspace(page: Page) {
  await page.getByRole("button", { name: "Close workspace" }).click();
  await expect(page.getByTestId("workspace-panel")).toHaveCount(0);
}

test.beforeEach(async ({ page }) => {
  await installMockBridge(page);
  await page.goto("/");
  await page.getByTestId("channel-general").click();
  await waitForLiveSubscription(page);
});

test("reader exercises Markdown, PDF, and malformed content with deterministic evidence", async ({
  page,
}) => {
  const markdownUrl = "https://example.com/native-qa/handoff.md";
  await page.route(markdownUrl, (route) =>
    route.fulfill({
      body: "# Agent Lab\n\nEvidence-backed Markdown.",
      contentType: "text/markdown",
    }),
  );
  await emitAttachment(page, "handoff.md", "text/markdown", markdownUrl);
  await openFileCard(page, "handoff.md");
  await expect(page.getByTestId("workspace-markdown-preview")).toContainText(
    "Agent Lab",
  );
  await page.screenshot({
    path: "test-results/native-qa/workspace-markdown.png",
  });
  await closeWorkspace(page);

  const pdfUrl = "https://example.com/native-qa/brief.pdf";
  await page.route(pdfUrl, (route) =>
    route.fulfill({
      body: onePagePdf("Agent Lab PDF evidence"),
      contentType: "application/pdf",
    }),
  );
  await emitAttachment(page, "brief.pdf", "application/pdf", pdfUrl);
  await openFileCard(page, "brief.pdf");
  await expect(page.getByTestId("workspace-pdf-preview")).toBeVisible();
  await expect(page.getByTestId("workspace-pdf-page")).toBeVisible();
  await page.screenshot({
    path: "test-results/native-qa/workspace-pdf.png",
  });
  await closeWorkspace(page);

  const malformedUrl = "https://example.com/native-qa/malformed.pdf";
  await page.route(malformedUrl, (route) =>
    route.fulfill({
      body: "not a PDF",
      contentType: "application/pdf",
    }),
  );
  await emitAttachment(page, "malformed.pdf", "application/pdf", malformedUrl);
  await openFileCard(page, "malformed.pdf");
  await expect(page.getByTestId("workspace-artifact-error")).toContainText(
    "valid PDF header",
  );
  await page.screenshot({
    path: "test-results/native-qa/workspace-malformed.png",
  });
});

test("reader previews DOCX, XLSX, and PPTX while keeping unknown formats unsupported", async ({
  page,
}) => {
  const fixtures = [
    [
      "brief.docx",
      "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ],
    [
      "model.xlsx",
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ],
    [
      "deck.pptx",
      "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ],
  ] as const;

  for (const [filename, mime] of fixtures) {
    await emitAttachment(
      page,
      filename,
      mime,
      `https://example.com/native-qa/${filename}`,
    );
    await openFileCard(page, filename);
    await expect(
      page.getByTestId(`workspace-${filename.split(".").at(-1)}-preview`),
    ).toBeVisible();
    await expect(
      page
        .getByTestId("workspace-panel")
        .getByRole("button", { name: `Download ${filename}` }),
    ).toBeVisible();
    await closeWorkspace(page);
  }

  await emitAttachment(
    page,
    "archive.bin",
    "application/octet-stream",
    "https://example.com/native-qa/archive.bin",
  );
  await openFileCard(page, "archive.bin");
  await expect(
    page.getByTestId("workspace-artifact-unsupported"),
  ).toContainText("Preview not available yet");
  await closeWorkspace(page);

  await emitAttachment(
    page,
    "final-preview.pptx",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "https://example.com/native-qa/final-preview.pptx",
  );
  await openFileCard(page, "final-preview.pptx");
  await expect(page.getByText("Trusted agent collaboration")).toBeVisible();
  await page.waitForTimeout(250);
  await page.screenshot({
    path: "test-results/native-qa/workspace-office-preview.png",
  });
});

test("image context menu opens the image in the artifact workspace", async ({
  page,
}) => {
  const imageUrl = "https://example.com/native-qa/evidence.png";
  const onePixelPng = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
    "base64",
  );
  await page.route(imageUrl, (route) =>
    route.fulfill({ body: onePixelPng, contentType: "image/png" }),
  );
  await emitAttachment(page, "evidence.png", "image/png", imageUrl, true);

  const trigger = page.getByTestId("message-image-lightbox-trigger").last();
  await expect(trigger).toBeVisible();
  await trigger.click({ button: "right" });
  const menu = page.locator("[data-image-context-menu]");
  await expect(menu).toBeVisible();
  await menu.getByRole("button", { name: "Open image in workspace" }).click();

  await expect(page.getByTestId("workspace-image-preview")).toBeVisible();
  await page.screenshot({
    path: "test-results/native-qa/workspace-image.png",
  });
});

test("browser panel covers navigation, blocked-embed fallback, restart, and keyboard close", async ({
  page,
}) => {
  await page.evaluate(() => {
    const emit = (
      window as Window & {
        __BUZZ_E2E_EMIT_MOCK_MESSAGE__?: (input: {
          channelName: string;
          content: string;
        }) => void;
      }
    ).__BUZZ_E2E_EMIT_MOCK_MESSAGE__;
    if (!emit) throw new Error("Mock message emitter is unavailable.");
    emit({
      channelName: "general",
      content: "Blocked docs: https://example.com/blocked",
    });
  });

  const link = page.getByRole("link", {
    name: "https://example.com/blocked",
  });
  await link.evaluate((element) =>
    element.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
    ),
  );
  await page
    .locator("[data-link-context-menu]")
    .getByRole("button", { name: "Open in Buzz browser" })
    .click();

  const frame = page.getByTestId("workspace-browser-frame");
  await expect(frame).toHaveAttribute("src", "https://example.com/blocked");
  await expect(
    page.getByText("Some sites block embedded browsing."),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open in system browser" }),
  ).toBeVisible();

  const address = page.getByRole("textbox", { name: "Browser address" });
  await address.fill("example.org/second");
  await address.press("Enter");
  await expect(frame).toHaveAttribute("src", "https://example.org/second");
  const panel = page.getByTestId("workspace-panel");
  const mainContent = page.getByTestId("main-content-pane");
  const [mainBox, panelBox] = await Promise.all([
    mainContent.boundingBox(),
    panel.boundingBox(),
  ]);
  expect(mainBox).not.toBeNull();
  expect(panelBox).not.toBeNull();
  expect((mainBox?.x ?? 0) + (mainBox?.width ?? 0)).toBeLessThanOrEqual(
    (panelBox?.x ?? 0) + 1,
  );
  await expect(panel).toHaveCSS("position", "relative");
  await expect(
    page.getByRole("button", { name: "Resize workspace" }),
  ).toBeVisible();
  await panel.getByRole("button", { name: "Back", exact: true }).click();
  await expect(frame).toHaveAttribute("src", "https://example.com/blocked");
  await panel.getByRole("button", { name: "Forward" }).click();
  await expect(frame).toHaveAttribute("src", "https://example.org/second");
  await panel.getByRole("button", { name: "Reload" }).click();

  await page.screenshot({
    path: "test-results/native-qa/workspace-browser.png",
  });
  await address.focus();
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("workspace-panel")).toHaveCount(0);

  await page.reload();
  await expect(page.getByTestId("workspace-panel")).toHaveCount(0);
});

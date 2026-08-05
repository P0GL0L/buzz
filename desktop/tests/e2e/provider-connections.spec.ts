import { expect, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";
import { openSettings } from "../helpers/settings";

test("provider settings distinguish install and OAuth connection state", async ({
  page,
}) => {
  await installMockBridge(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await openSettings(page, "agents");

  const providers = page.getByTestId("provider-connection-list");
  await expect(providers).toBeVisible();

  const grok = page.getByTestId("provider-connection-xai");
  await expect(grok.getByText("Grok Build")).toBeVisible();
  await expect(grok.getByText("Ready to connect")).toBeVisible();
  await expect(grok.getByRole("button", { name: "Sign in" })).toBeVisible();

  const antigravity = page.getByTestId("provider-connection-google-personal");
  await expect(antigravity.getByText("Signed in — verify")).toBeVisible();
  await expect(
    antigravity.getByText("Authentication: signed in"),
  ).toBeVisible();
  await expect(
    antigravity.getByRole("button", { name: "Verify", exact: true }),
  ).toBeVisible();
  await expect(
    antigravity.getByRole("button", { name: "Disconnect" }),
  ).toBeVisible();

  const gemini = page.getByTestId("provider-connection-google-enterprise");
  await expect(gemini.getByText("Not installed")).toBeVisible();
  await expect(
    gemini.getByRole("button", { name: "Install guide" }),
  ).toBeVisible();
});

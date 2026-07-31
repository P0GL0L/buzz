import { expect, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";
import { openSettings } from "../helpers/settings";

test("skills settings search and filter relay-safe observations", async ({
  page,
}) => {
  await installMockBridge(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await openSettings(page, "skills");

  const registry = page.getByTestId("skill-registry-list");
  await expect(registry).toBeVisible();
  await expect(
    registry.getByRole("heading", { name: "Browser control" }),
  ).toBeVisible();
  await expect(
    registry.getByRole("heading", { name: "Spreadsheets" }),
  ).toBeVisible();
  await expect(page.getByText("1 routable")).toBeVisible();

  await page.getByRole("searchbox", { name: "Search skills" }).fill("workbook");
  await expect(
    registry.getByRole("heading", { name: "Spreadsheets" }),
  ).toBeVisible();
  await expect(
    registry.getByRole("heading", { name: "Browser control" }),
  ).toBeHidden();
  await expect(registry.getByText("expired")).toBeVisible();

  await page.getByLabel("Installation state").selectOption("callable");
  await expect(
    registry.getByText("No indexed skills match these filters."),
  ).toBeVisible();
});

import assert from "node:assert/strict";
import test from "node:test";

import { filterSkillRecords } from "./skillRegistryLogic.ts";

function skill(overrides = {}) {
  return {
    abstractRequirements: ["browser access"],
    availability: "available",
    capabilities: ["inspect a page"],
    displayName: "Browser",
    expired: false,
    installationState: "callable",
    observedAt: "2026-07-30T00:00:00Z",
    owningAgent:
      "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    registryVersion: 1,
    routable: true,
    runtimeClass: "codex",
    skillId: "codex:browser",
    ...overrides,
  };
}

test("filters relay-safe records across query and structured fields", () => {
  const records = [
    skill(),
    skill({
      availability: "unknown",
      capabilities: ["prepare a workbook"],
      displayName: "Spreadsheets",
      installationState: "installed",
      routable: false,
      runtimeClass: "hermes",
      skillId: "hermes:spreadsheets",
    }),
  ];
  assert.deepEqual(
    filterSkillRecords(records, {
      availability: "unknown",
      query: "workbook",
      runtime: "hermes",
      state: "installed",
    }).map((record) => record.skillId),
    ["hermes:spreadsheets"],
  );
});

test("all filters preserve the full searchable relay projection", () => {
  const records = [skill(), skill({ skillId: "codex:pdf" })];
  assert.equal(
    filterSkillRecords(records, {
      availability: "all",
      query: "",
      runtime: "all",
      state: "all",
    }).length,
    2,
  );
});

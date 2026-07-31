export type SkillRecord = {
  abstractRequirements: string[];
  availability: "available" | "degraded" | "unavailable" | "unknown";
  capabilities: string[];
  displayName: string;
  expired: boolean;
  installationState:
    | "catalogued"
    | "installed"
    | "callable"
    | "degraded"
    | "unreachable";
  observedAt: string;
  owningAgent: string;
  registryVersion: number;
  routable: boolean;
  runtimeClass: string;
  skillId: string;
};

export type SkillFilters = {
  availability: string;
  query: string;
  runtime: string;
  state: string;
};

export function filterSkillRecords(
  records: SkillRecord[],
  filters: SkillFilters,
): SkillRecord[] {
  const needle = filters.query.trim().toLowerCase();
  return records.filter((skill) => {
    const text = [
      skill.skillId,
      skill.displayName,
      skill.owningAgent,
      skill.runtimeClass,
      ...skill.capabilities,
      ...skill.abstractRequirements,
    ]
      .join(" ")
      .toLowerCase();
    return (
      (!needle || text.includes(needle)) &&
      (filters.state === "all" || skill.installationState === filters.state) &&
      (filters.availability === "all" ||
        skill.availability === filters.availability) &&
      (filters.runtime === "all" || skill.runtimeClass === filters.runtime)
    );
  });
}

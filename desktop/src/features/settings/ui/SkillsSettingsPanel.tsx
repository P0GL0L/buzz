import * as React from "react";
import { invoke } from "@tauri-apps/api/core";
import { CircleAlert, RefreshCw, Route, Search } from "lucide-react";

import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/button";
import { SkillActions } from "./SkillActions";
import { filterSkillRecords, type SkillRecord } from "./skillRegistryLogic";
import { SkillResultReview } from "./SkillResultReview";

type SkillRegistryView = {
  version: number;
  generatedAt: string;
  registryDigest: string;
  registryRevision: string;
  registryVersion: number;
  ttlSeconds: number;
  skills: SkillRecord[];
};

function shortOwner(owner: string): string {
  return `${owner.slice(0, 8)}…${owner.slice(-6)}`;
}

function tone(record: SkillRecord): string {
  if (record.routable) return "bg-emerald-500/15 text-emerald-600";
  if (record.expired || record.availability === "unknown") {
    return "bg-amber-500/15 text-amber-600";
  }
  return "bg-muted text-muted-foreground";
}

export function SkillsSettingsPanel() {
  const [registry, setRegistry] = React.useState<SkillRegistryView | null>(
    null,
  );
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [query, setQuery] = React.useState("");
  const [state, setState] = React.useState("all");
  const [availability, setAvailability] = React.useState("all");
  const [runtime, setRuntime] = React.useState("all");

  const load = React.useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setRegistry(await invoke<SkillRegistryView>("list_skill_registry"));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    void load();
  }, [load]);

  const runtimes = React.useMemo(
    () =>
      [
        ...new Set(registry?.skills.map((skill) => skill.runtimeClass) ?? []),
      ].sort(),
    [registry],
  );
  const skills = React.useMemo(() => {
    return filterSkillRecords(registry?.skills ?? [], {
      availability,
      query,
      runtime,
      state,
    });
  }, [availability, query, registry, runtime, state]);

  return (
    <section aria-labelledby="skills-settings-title">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2
            className="text-lg font-semibold tracking-tight"
            id="skills-settings-title"
          >
            Skills
          </h2>
          <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
            Search the signed relay-safe catalog. Complete inventories, paths,
            permissions, and connector details remain with their owning hosts.
          </p>
        </div>
        <Button
          onClick={() => void load()}
          size="sm"
          type="button"
          variant="outline"
        >
          <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
          Refresh
        </Button>
      </div>

      <div className="mt-5 grid gap-3 md:grid-cols-[minmax(14rem,1fr)_repeat(3,minmax(9rem,auto))]">
        <label className="relative">
          <span className="sr-only">Search skills</span>
          <Search className="pointer-events-none absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            className="h-9 w-full rounded-lg border border-border bg-background pl-9 pr-3 text-sm"
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search capability, skill, or owner"
            type="search"
            value={query}
          />
        </label>
        <select
          aria-label="Installation state"
          className="h-9 rounded-lg border border-border bg-background px-3 text-sm"
          onChange={(event) => setState(event.target.value)}
          value={state}
        >
          <option value="all">All states</option>
          {[
            "catalogued",
            "installed",
            "callable",
            "degraded",
            "unreachable",
          ].map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
        <select
          aria-label="Availability"
          className="h-9 rounded-lg border border-border bg-background px-3 text-sm"
          onChange={(event) => setAvailability(event.target.value)}
          value={availability}
        >
          <option value="all">All availability</option>
          {["available", "degraded", "unavailable", "unknown"].map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
        <select
          aria-label="Runtime"
          className="h-9 rounded-lg border border-border bg-background px-3 text-sm"
          onChange={(event) => setRuntime(event.target.value)}
          value={runtime}
        >
          <option value="all">All runtimes</option>
          {runtimes.map((value) => (
            <option key={value} value={value}>
              {value}
            </option>
          ))}
        </select>
      </div>

      {error ? (
        <div className="mt-4 flex gap-3 rounded-2xl bg-destructive/10 px-4 py-4 text-sm text-destructive">
          <CircleAlert className="mt-0.5 h-4 w-4 shrink-0" />
          <div>
            <p>{error}</p>
            <p className="mt-1 text-muted-foreground">
              Run the owning host&apos;s read-only scan and relay-safe
              validation, then refresh. Buzz will not invent a replacement
              inventory.
            </p>
          </div>
        </div>
      ) : null}

      {!error && registry ? (
        <>
          <div className="mt-4 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
            <span>Revision {registry.registryRevision.slice(7, 19)}</span>
            <span>{registry.skills.length} indexed</span>
            <span>
              {registry.skills.filter((skill) => skill.routable).length}{" "}
              routable
            </span>
            <span>
              Observed {new Date(registry.generatedAt).toLocaleString()}
            </span>
          </div>
          <div className="mt-3 space-y-3" data-testid="skill-registry-list">
            {skills.map((skill) => (
              <article
                className="rounded-2xl border border-border/60 bg-muted/20 px-4 py-4"
                key={`${skill.skillId}:${skill.owningAgent}`}
              >
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="text-sm font-medium">
                        {skill.displayName}
                      </h3>
                      <span
                        className={cn(
                          "rounded-md px-2 py-0.5 text-xs",
                          tone(skill),
                        )}
                      >
                        {skill.routable
                          ? "routable"
                          : skill.expired
                            ? "expired"
                            : skill.availability}
                      </span>
                    </div>
                    <p className="mt-1 break-all text-xs text-muted-foreground">
                      {skill.skillId}
                    </p>
                    <p className="mt-2 text-sm text-muted-foreground">
                      {skill.capabilities.join(" · ")}
                    </p>
                  </div>
                  <div className="shrink-0 text-right text-xs text-muted-foreground">
                    <p>{skill.runtimeClass}</p>
                    <p title={skill.owningAgent}>
                      {shortOwner(skill.owningAgent)}
                    </p>
                    <p>{new Date(skill.observedAt).toLocaleString()}</p>
                  </div>
                </div>
                {skill.routable ? (
                  <p className="mt-3 flex items-center gap-1.5 text-xs text-emerald-600">
                    <Route className="h-3.5 w-3.5" />
                    Current callable evidence; route to the named owner.
                  </p>
                ) : null}
                <SkillActions skill={skill} />
              </article>
            ))}
            {skills.length === 0 ? (
              <p className="rounded-2xl bg-muted/20 px-4 py-5 text-sm text-muted-foreground">
                No indexed skills match these filters.
              </p>
            ) : null}
          </div>
          <SkillResultReview />
        </>
      ) : null}
    </section>
  );
}

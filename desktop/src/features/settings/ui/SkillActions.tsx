import * as React from "react";
import {
  CheckCircle2,
  Info,
  MessageSquareText,
  Route,
  ShieldCheck,
  X,
} from "lucide-react";
import { toast } from "sonner";

import { useChannelsQuery } from "@/features/channels/hooks";
import { invokeTauri } from "@/shared/api/tauri";
import { relayClient } from "@/shared/api/relayClient";
import { Button } from "@/shared/ui/button";
import type { SkillRecord } from "./skillRegistryLogic";

type SkillAction = "owner-detail" | "route" | "verify";

type SkillRequestView = {
  action: SkillAction;
  content: string;
  correlationId: string;
  expectedOwner: string;
  skillId: string;
  version: 1;
};

const DEFAULT_TASKS: Record<SkillAction, string> = {
  route: "Run this bounded task and return the requested evidence.",
  verify:
    "Run a bounded non-destructive verification task and report runtime discovery, dependency probes, and the signed result.",
  "owner-detail":
    "Return only the additional non-secret requirements needed to route a real bounded task.",
};

export function SkillActions({ skill }: { skill: SkillRecord }) {
  const channelsQuery = useChannelsQuery();
  const channels = React.useMemo(
    () =>
      (channelsQuery.data ?? []).filter(
        (channel) =>
          channel.isMember &&
          channel.archivedAt == null &&
          channel.channelType !== "forum",
      ),
    [channelsQuery.data],
  );
  const [expanded, setExpanded] = React.useState(false);
  const [action, setAction] = React.useState<SkillAction | null>(null);
  const [channelId, setChannelId] = React.useState("");
  const [task, setTask] = React.useState(DEFAULT_TASKS.route);
  const [evidence, setEvidence] = React.useState("completion summary");
  const [sending, setSending] = React.useState(false);
  const [lastCorrelation, setLastCorrelation] = React.useState<string | null>(
    null,
  );

  React.useEffect(() => {
    if (!channelId && channels[0]) setChannelId(channels[0].id);
  }, [channelId, channels]);

  function begin(next: SkillAction) {
    setAction(next);
    setTask(DEFAULT_TASKS[next]);
  }

  async function send() {
    if (!action || !channelId) {
      toast.error("Choose a channel before sending the skill request");
      return;
    }
    setSending(true);
    try {
      const request = await invokeTauri<SkillRequestView>(
        "build_skill_request",
        {
          action,
          skillId: skill.skillId,
          expectedOwner: skill.owningAgent,
          task,
          requiredEvidence: evidence
            .split(",")
            .map((value) => value.trim())
            .filter(Boolean),
          correlationId: null,
        },
      );
      await relayClient.sendMessage(channelId, request.content, [
        request.expectedOwner,
      ]);
      setLastCorrelation(request.correlationId);
      setAction(null);
      toast.success("Signed skill request sent", {
        description: `Correlation ${request.correlationId}`,
      });
    } catch (cause) {
      toast.error("Skill request was not sent", {
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="mt-3 border-t border-border/50 pt-3">
      <div className="flex flex-wrap gap-1.5">
        <Button
          onClick={() => setExpanded((value) => !value)}
          size="xs"
          type="button"
          variant="ghost"
        >
          <Info />
          Inspect
        </Button>
        <Button
          onClick={() => begin("verify")}
          size="xs"
          type="button"
          variant="outline"
        >
          <ShieldCheck />
          Verify
        </Button>
        <Button
          disabled={!skill.routable}
          onClick={() => begin("route")}
          size="xs"
          type="button"
          variant="outline"
        >
          <Route />
          Route
        </Button>
        <Button
          onClick={() => begin("owner-detail")}
          size="xs"
          type="button"
          variant="outline"
        >
          <MessageSquareText />
          Ask owner
        </Button>
      </div>

      {expanded ? (
        <dl className="mt-3 grid gap-2 rounded-xl bg-background/70 p-3 text-xs sm:grid-cols-2">
          <div>
            <dt className="font-medium">Capabilities</dt>
            <dd className="mt-1 text-muted-foreground">
              {skill.capabilities.join(" · ") || "No capability summary"}
            </dd>
          </div>
          <div>
            <dt className="font-medium">Abstract requirements</dt>
            <dd className="mt-1 text-muted-foreground">
              {skill.abstractRequirements.join(" · ") ||
                "No relay-safe requirements"}
            </dd>
          </div>
          <div>
            <dt className="font-medium">Owner</dt>
            <dd className="mt-1 break-all text-muted-foreground">
              {skill.owningAgent}
            </dd>
          </div>
          <div>
            <dt className="font-medium">Observation</dt>
            <dd className="mt-1 text-muted-foreground">
              {skill.installationState} · {skill.availability}
              {skill.expired ? " · expired" : ""}
            </dd>
          </div>
        </dl>
      ) : null}

      {action ? (
        <div className="mt-3 space-y-3 rounded-xl border bg-background p-3">
          <div className="flex items-center justify-between gap-3">
            <p className="text-xs font-medium">
              {action === "route"
                ? "Route bounded task"
                : action === "verify"
                  ? "Request verification"
                  : "Request owner details"}
            </p>
            <Button
              aria-label="Close skill request"
              onClick={() => setAction(null)}
              size="icon-xs"
              type="button"
              variant="ghost"
            >
              <X />
            </Button>
          </div>
          <label className="block text-xs">
            <span className="mb-1 block font-medium">Private channel</span>
            <select
              className="h-9 w-full rounded-lg border bg-background px-3"
              onChange={(event) => setChannelId(event.target.value)}
              value={channelId}
            >
              <option value="">Choose a channel</option>
              {channels.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  {channel.name}
                </option>
              ))}
            </select>
          </label>
          <label className="block text-xs">
            <span className="mb-1 block font-medium">Task bounds</span>
            <textarea
              className="min-h-24 w-full rounded-lg border bg-background px-3 py-2"
              maxLength={65_536}
              onChange={(event) => setTask(event.target.value)}
              value={task}
            />
          </label>
          <label className="block text-xs">
            <span className="mb-1 block font-medium">
              Required evidence, comma-separated
            </span>
            <input
              className="h-9 w-full rounded-lg border bg-background px-3"
              onChange={(event) => setEvidence(event.target.value)}
              value={evidence}
            />
          </label>
          <div className="flex justify-end">
            <Button
              disabled={sending || !channelId || !task.trim()}
              onClick={() => void send()}
              size="sm"
              type="button"
            >
              {sending ? "Signing…" : "Sign and send"}
            </Button>
          </div>
        </div>
      ) : null}

      {lastCorrelation ? (
        <p className="mt-2 flex items-center gap-1.5 text-xs text-emerald-600">
          <CheckCircle2 className="h-3.5 w-3.5" />
          Request sent · {lastCorrelation}
        </p>
      ) : null}
    </div>
  );
}

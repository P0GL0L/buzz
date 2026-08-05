import * as React from "react";
import { BadgeCheck, ScanSearch } from "lucide-react";
import { toast } from "sonner";

import { invokeTauri } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";

type SkillResultReviewView = {
  completedAt: string;
  completionState: string;
  correlationId: string;
  evidenceSummary: string;
  eventId: string;
  executionHostClass: string;
  failureReason?: string;
  proofsComplete: boolean;
  signer: string;
  skillHash: string;
  skillId: string;
  skillVersion: string;
  signatureValid: boolean;
  version: 1;
};

export function SkillResultReview() {
  const [open, setOpen] = React.useState(false);
  const [eventJson, setEventJson] = React.useState("");
  const [correlationId, setCorrelationId] = React.useState("");
  const [review, setReview] = React.useState<SkillResultReviewView | null>(
    null,
  );
  const [checking, setChecking] = React.useState(false);

  async function verify() {
    setChecking(true);
    setReview(null);
    try {
      setReview(
        await invokeTauri<SkillResultReviewView>("review_signed_skill_result", {
          eventJson,
          expectedCorrelationId: correlationId.trim() || null,
        }),
      );
    } catch (cause) {
      toast.error("Signed result verification failed", {
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="mt-4 rounded-2xl border border-border/60 bg-muted/20 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p className="text-sm font-medium">Signed result review</p>
          <p className="mt-1 text-xs text-muted-foreground">
            Validate an owner-signed skill-result/v1 event and its correlation
            before trusting execution evidence.
          </p>
        </div>
        <Button
          onClick={() => setOpen((value) => !value)}
          size="sm"
          type="button"
          variant="outline"
        >
          <ScanSearch />
          {open ? "Close reviewer" : "Review result"}
        </Button>
      </div>
      {open ? (
        <div className="mt-3 space-y-3">
          <label className="block text-xs">
            <span className="mb-1 block font-medium">
              Expected correlation ID
            </span>
            <input
              className="h-9 w-full rounded-lg border bg-background px-3"
              onChange={(event) => setCorrelationId(event.target.value)}
              placeholder="Optional UUID"
              value={correlationId}
            />
          </label>
          <label className="block text-xs">
            <span className="mb-1 block font-medium">
              Signed Nostr event JSON
            </span>
            <textarea
              className="min-h-36 w-full rounded-lg border bg-background px-3 py-2 font-mono text-3xs"
              maxLength={65_536}
              onChange={(event) => setEventJson(event.target.value)}
              value={eventJson}
            />
          </label>
          <Button
            disabled={checking || !eventJson.trim()}
            onClick={() => void verify()}
            size="sm"
            type="button"
          >
            {checking ? "Verifying…" : "Verify signature and result"}
          </Button>
          {review ? (
            <div className="rounded-xl bg-emerald-500/10 p-3 text-xs">
              <p className="flex items-center gap-1.5 font-medium text-emerald-700 dark:text-emerald-300">
                <BadgeCheck className="h-4 w-4" />
                Valid signed result · {review.completionState}
              </p>
              <dl className="mt-2 grid gap-2 sm:grid-cols-2">
                <div>
                  <dt className="font-medium">Skill</dt>
                  <dd className="break-all text-muted-foreground">
                    {review.skillId} · {review.skillVersion}
                  </dd>
                </div>
                <div>
                  <dt className="font-medium">Runtime</dt>
                  <dd className="text-muted-foreground">
                    {review.executionHostClass}
                  </dd>
                </div>
                <div>
                  <dt className="font-medium">Correlation</dt>
                  <dd className="break-all text-muted-foreground">
                    {review.correlationId}
                  </dd>
                </div>
                <div>
                  <dt className="font-medium">Proofs</dt>
                  <dd className="text-muted-foreground">
                    {review.proofsComplete ? "complete" : "incomplete"}
                  </dd>
                </div>
              </dl>
              <p className="mt-2 text-muted-foreground">
                {review.evidenceSummary}
              </p>
              {review.failureReason ? (
                <p className="mt-1 text-destructive">{review.failureReason}</p>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

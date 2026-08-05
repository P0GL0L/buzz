import * as React from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertCircle,
  CheckCircle2,
  ExternalLink,
  LoaderCircle,
  PlugZap,
  Unplug,
} from "lucide-react";

import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/button";

type ProviderFailure = {
  code: string;
  message: string;
  recoverable: boolean;
};

type ProviderConnectionRecord = {
  version: number;
  providerId: string;
  runtimeId: string;
  label: string;
  authMethod: string;
  installState: "installed" | "not_installed";
  authenticationState:
    | "authenticated"
    | "unknown"
    | "disconnected"
    | "unavailable";
  availability: string;
  verificationTime: string | null;
  expirationTime: string | null;
  storageScope: string;
  failure: ProviderFailure | null;
  installUrl: string;
};

function statusLabel(record: ProviderConnectionRecord): string {
  switch (record.availability) {
    case "verified":
      return "Connected";
    case "authenticated_unverified":
      return "Signed in — verify";
    case "pending_consent":
      return "Sign-in opened";
    case "degraded":
      return "Needs attention";
    case "disconnected":
      return "Disconnected";
    case "installed_unverified":
      return "Ready to connect";
    case "not_installed":
      return "Not installed";
    default:
      return record.availability.replaceAll("_", " ");
  }
}

function statusTone(record: ProviderConnectionRecord): string {
  if (record.availability === "verified") {
    return "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400";
  }
  if (
    record.availability === "pending_consent" ||
    record.availability === "authenticated_unverified" ||
    record.availability === "installed_unverified"
  ) {
    return "bg-amber-500/15 text-amber-600 dark:text-amber-400";
  }
  return "bg-muted text-muted-foreground";
}

/**
 * Non-secret account-routing status for provider-owned OAuth runtimes.
 *
 * Buzz never receives or renders tokens. Sign in launches the provider's
 * official interactive login. Verify runs a bounded no-tools provider task and
 * stores only its pass/fail status. Disconnect only removes Buzz's permission
 * to route work and leaves provider-owned credentials alone.
 */
export function ProviderConnectionsPanel() {
  const [records, setRecords] = React.useState<ProviderConnectionRecord[]>([]);
  const [loading, setLoading] = React.useState(true);
  const [action, setAction] = React.useState<string | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  const load = React.useCallback(async () => {
    setError(null);
    const next = await invoke<ProviderConnectionRecord[]>(
      "list_provider_connections",
    );
    setRecords(next);
  }, []);

  React.useEffect(() => {
    let active = true;
    void load()
      .catch((cause) => {
        if (active) {
          setError(cause instanceof Error ? cause.message : String(cause));
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [load]);

  const run = async (
    providerId: string,
    operation: "connect" | "verify" | "disconnect",
  ) => {
    setAction(providerId);
    setError(null);
    try {
      const command = {
        connect: "connect_provider_connection",
        verify: "verify_provider_connection",
        disconnect: "disconnect_provider_connection",
      }[operation];
      await invoke(command, { providerId });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setAction(null);
    }
  };

  return (
    <section aria-labelledby="provider-connections-title">
      <div className="mb-3">
        <h2
          className="text-lg font-semibold tracking-tight"
          id="provider-connections-title"
        >
          Provider connections
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Buzz detects provider-owned sign-in evidence without reading it.
          Verify runs one bounded no-tools response before the runtime can be
          treated as connected. Buzz stores status and routing metadata, never
          tokens.
        </p>
      </div>

      {loading ? (
        <div className="rounded-2xl bg-muted/20 px-4 py-4 text-sm text-muted-foreground">
          Checking provider connections...
        </div>
      ) : (
        <div className="space-y-3" data-testid="provider-connection-list">
          {records.map((record) => {
            const working = action === record.providerId;
            const canDisconnect = [
              "verified",
              "authenticated_unverified",
              "pending_consent",
              "degraded",
              "expired",
            ].includes(record.availability);
            const canVerify = [
              "verified",
              "authenticated_unverified",
              "pending_consent",
              "degraded",
              "expired",
            ].includes(record.availability);
            return (
              <article
                className="rounded-2xl border border-border/60 bg-muted/20 px-4 py-4"
                data-testid={`provider-connection-${record.providerId}`}
                key={record.providerId}
              >
                <div className="flex flex-wrap items-start justify-between gap-4">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      {record.availability === "verified" ? (
                        <CheckCircle2 className="h-4 w-4 text-emerald-500" />
                      ) : record.failure ? (
                        <AlertCircle className="h-4 w-4 text-destructive" />
                      ) : (
                        <PlugZap className="h-4 w-4 text-muted-foreground" />
                      )}
                      <h3 className="text-sm font-medium">{record.label}</h3>
                      <span
                        className={cn(
                          "rounded-md px-2 py-0.5 text-xs font-medium capitalize",
                          statusTone(record),
                        )}
                      >
                        {statusLabel(record)}
                      </span>
                    </div>
                    <p className="mt-2 text-sm text-muted-foreground">
                      Authentication:{" "}
                      {record.authenticationState === "authenticated"
                        ? "signed in"
                        : record.authenticationState.replaceAll("_", " ")}
                      {" · "}Runtime: {record.runtimeId} ·{" "}
                      {record.authMethod.replaceAll("-", " ")}
                    </p>
                    {record.verificationTime ? (
                      <p className="mt-1 text-xs text-muted-foreground">
                        Verified {record.verificationTime}
                      </p>
                    ) : null}
                    {record.failure ? (
                      <p className="mt-2 text-sm text-destructive">
                        {record.failure.message}
                      </p>
                    ) : null}
                  </div>

                  <div className="flex shrink-0 flex-wrap gap-2">
                    {record.installState === "not_installed" ? (
                      <Button
                        onClick={() => void openUrl(record.installUrl)}
                        size="sm"
                        type="button"
                        variant="outline"
                      >
                        <ExternalLink className="h-4 w-4" />
                        Install guide
                      </Button>
                    ) : canVerify ? (
                      <Button
                        disabled={working}
                        onClick={() => void run(record.providerId, "verify")}
                        size="sm"
                        type="button"
                        variant="outline"
                      >
                        {working ? (
                          <LoaderCircle className="h-4 w-4 animate-spin" />
                        ) : (
                          <CheckCircle2 className="h-4 w-4" />
                        )}
                        {record.availability === "verified"
                          ? "Verify again"
                          : "Verify"}
                      </Button>
                    ) : (
                      <Button
                        disabled={working}
                        onClick={() => void run(record.providerId, "connect")}
                        size="sm"
                        type="button"
                        variant="outline"
                      >
                        {working ? (
                          <LoaderCircle className="h-4 w-4 animate-spin" />
                        ) : (
                          <PlugZap className="h-4 w-4" />
                        )}
                        {record.availability === "disconnected"
                          ? "Sign in again"
                          : "Sign in"}
                      </Button>
                    )}
                    {canDisconnect ? (
                      <Button
                        disabled={working}
                        onClick={() =>
                          void run(record.providerId, "disconnect")
                        }
                        size="sm"
                        type="button"
                        variant="ghost"
                      >
                        <Unplug className="h-4 w-4" />
                        Disconnect
                      </Button>
                    ) : null}
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      )}

      {error ? (
        <p className="mt-3 rounded-2xl bg-destructive/10 px-4 py-4 text-sm text-destructive">
          {error}
        </p>
      ) : null}
    </section>
  );
}

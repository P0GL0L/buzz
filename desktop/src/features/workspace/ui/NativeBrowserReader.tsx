import * as React from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowLeft,
  ArrowRight,
  Camera,
  ExternalLink,
  FileSearch,
  ListRestart,
  Loader2,
  RefreshCw,
  ShieldCheck,
  Square,
  Trash2,
  X,
} from "lucide-react";
import { toast } from "sonner";

import { invokeTauri } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";
import { requestOpenWorkspaceResource } from "../openWorkspaceResourceEvent";
import {
  boundsForElement,
  type BrowserPageExtraction,
  type NativeWorkspaceBrowserState,
} from "../lib/workspaceBrowser";
import {
  normalizeBrowserUrl,
  type BrowserWorkspaceResource,
} from "../lib/workspaceResource";

type BrowserMode = "connecting" | "native" | "fallback";

function fallbackState(initialUrl: string) {
  return {
    history: [initialUrl],
    index: 0,
    input: initialUrl,
    reloadKey: 0,
  };
}

export function NativeBrowserReader({
  resource,
}: {
  resource: BrowserWorkspaceResource;
}) {
  const initialUrl =
    normalizeBrowserUrl(resource.url) ?? "https://example.com/";
  const contentRef = React.useRef<HTMLDivElement>(null);
  const [mode, setMode] = React.useState<BrowserMode>("connecting");
  const [nativeState, setNativeState] =
    React.useState<NativeWorkspaceBrowserState | null>(null);
  const [fallback, setFallback] = React.useState(() =>
    fallbackState(initialUrl),
  );
  const [input, setInput] = React.useState(initialUrl);
  const [showAudit, setShowAudit] = React.useState(false);
  const [extracting, setExtracting] = React.useState(false);
  const currentUrl =
    mode === "native"
      ? (nativeState?.currentUrl ?? initialUrl)
      : (fallback.history[fallback.index] ?? initialUrl);

  const updateBounds = React.useCallback(() => {
    if (mode !== "native" || !contentRef.current) return;
    void invokeTauri("update_workspace_browser_bounds", {
      bounds: boundsForElement(contentRef.current),
    }).catch(() => {
      // Resize races with panel close are expected and need no user-facing error.
    });
  }, [mode]);

  React.useEffect(() => {
    const next = normalizeBrowserUrl(resource.url);
    if (!next) return;
    setInput(next);
    if (mode === "native") {
      void invokeTauri("navigate_workspace_browser", {
        url: next,
        actor: null,
      });
    } else {
      setFallback(fallbackState(next));
    }
  }, [mode, resource.url]);

  React.useEffect(() => {
    let active = true;
    let resizeObserver: ResizeObserver | undefined;
    let unlistenState: UnlistenFn | undefined;
    let unlistenPopup: UnlistenFn | undefined;
    let unlistenDownload: UnlistenFn | undefined;
    const timer = window.setTimeout(() => {
      const element = contentRef.current;
      if (!element || !active) return;
      void invokeTauri<NativeWorkspaceBrowserState>("open_workspace_browser", {
        url: initialUrl,
        bounds: boundsForElement(element),
        actor: null,
      })
        .then((state) => {
          if (!active || state.mode !== "native") {
            if (active) setMode("fallback");
            return;
          }
          setNativeState(state);
          setInput(state.currentUrl ?? initialUrl);
          setMode("native");
        })
        .catch(() => {
          if (active) setMode("fallback");
        });
    }, 240);

    void listen<NativeWorkspaceBrowserState>(
      "workspace-browser-state",
      (event) => {
        if (!active) return;
        setNativeState(event.payload);
        if (event.payload.currentUrl) setInput(event.payload.currentUrl);
      },
    ).then((dispose) => {
      unlistenState = dispose;
    });
    void listen<{ url: string; reason: string }>(
      "workspace-browser-popup-blocked",
      (event) => {
        toast.info("Browser popup blocked", {
          description: event.payload.reason,
          action: {
            label: "Open externally",
            onClick: () => void openUrl(event.payload.url),
          },
        });
      },
    ).then((dispose) => {
      unlistenPopup = dispose;
    });
    void listen<{ success: boolean; path?: string; filename?: string }>(
      "workspace-browser-download",
      (event) => {
        if (
          event.payload.success &&
          event.payload.path &&
          event.payload.filename
        ) {
          toast.success("Browser download saved to the ASV Buzz workspace");
          requestOpenWorkspaceResource({
            kind: "artifact",
            url: "",
            localPath: event.payload.path,
            filename: event.payload.filename,
          });
        } else {
          toast.error("Browser download failed");
        }
      },
    ).then((dispose) => {
      unlistenDownload = dispose;
    });

    if (contentRef.current) {
      resizeObserver = new ResizeObserver(() => updateBounds());
      resizeObserver.observe(contentRef.current);
    }
    return () => {
      active = false;
      window.clearTimeout(timer);
      resizeObserver?.disconnect();
      unlistenState?.();
      unlistenPopup?.();
      unlistenDownload?.();
      void invokeTauri("close_workspace_browser").catch(() => {});
    };
  }, [initialUrl, updateBounds]);

  React.useEffect(() => {
    if (mode !== "native" || !contentRef.current) return;
    const observer = new ResizeObserver(() => updateBounds());
    observer.observe(contentRef.current);
    updateBounds();
    return () => observer.disconnect();
  }, [mode, updateBounds]);

  function navigate(value: string) {
    const normalized = normalizeBrowserUrl(value);
    if (!normalized) {
      toast.error("Enter an HTTP or HTTPS address");
      return;
    }
    setInput(normalized);
    if (mode === "native") {
      void invokeTauri("navigate_workspace_browser", {
        url: normalized,
        actor: null,
      }).catch((error: unknown) =>
        toast.error(
          error instanceof Error ? error.message : "Navigation failed",
        ),
      );
      return;
    }
    setFallback((current) => {
      const history = [
        ...current.history.slice(0, current.index + 1),
        normalized,
      ];
      return {
        history,
        index: history.length - 1,
        input: normalized,
        reloadKey: current.reloadKey,
      };
    });
  }

  function nativeAction(command: string) {
    void invokeTauri(command, { actor: null }).catch((error: unknown) =>
      toast.error(
        error instanceof Error ? error.message : "Browser action failed",
      ),
    );
  }

  const canGoBack =
    mode === "native"
      ? (nativeState?.historyIndex ?? 0) > 0
      : fallback.index > 0;
  const canGoForward =
    mode === "native"
      ? (nativeState?.historyIndex ?? 0) <
        (nativeState?.history.length ?? 1) - 1
      : fallback.index < fallback.history.length - 1;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <form
        className="flex shrink-0 items-center gap-1.5 border-b border-border/70 px-3 py-2"
        onSubmit={(event) => {
          event.preventDefault();
          navigate(input);
        }}
      >
        <Button
          aria-label="Back"
          disabled={!canGoBack}
          onClick={() => {
            if (mode === "native") {
              nativeAction("workspace_browser_back");
            } else {
              setFallback((current) => {
                const index = Math.max(0, current.index - 1);
                setInput(current.history[index] ?? input);
                return { ...current, index };
              });
            }
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ArrowLeft />
        </Button>
        <Button
          aria-label="Forward"
          disabled={!canGoForward}
          onClick={() => {
            if (mode === "native") {
              nativeAction("workspace_browser_forward");
            } else {
              setFallback((current) => {
                const index = Math.min(
                  current.history.length - 1,
                  current.index + 1,
                );
                setInput(current.history[index] ?? input);
                return { ...current, index };
              });
            }
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ArrowRight />
        </Button>
        <Button
          aria-label="Reload"
          onClick={() => {
            if (mode === "native") {
              nativeAction("reload_workspace_browser");
            } else {
              setFallback((current) => ({
                ...current,
                reloadKey: current.reloadKey + 1,
              }));
            }
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <RefreshCw />
        </Button>
        {mode === "native" && nativeState?.loading ? (
          <Button
            aria-label="Stop loading"
            onClick={() => nativeAction("stop_workspace_browser")}
            size="icon-xs"
            type="button"
            variant="ghost"
          >
            <Square />
          </Button>
        ) : null}
        <input
          aria-label="Browser address"
          className="h-8 min-w-0 flex-1 rounded-lg border border-input/50 bg-muted/35 px-3 text-sm outline-hidden focus:border-ring focus:ring-1 focus:ring-ring"
          onChange={(event) => setInput(event.target.value)}
          spellCheck={false}
          value={input}
        />
        <span
          className="hidden max-w-36 items-center gap-1.5 truncate rounded-full bg-emerald-500/10 px-2 py-1 text-3xs text-emerald-700 sm:flex dark:text-emerald-300"
          data-testid="workspace-browser-active-agent"
          title={nativeState?.activeAgent ?? "Human control"}
        >
          <ShieldCheck className="h-3 w-3" />
          {nativeState?.activeAgent ?? "Human control"}
        </span>
        <Button
          aria-label="Browser action history"
          onClick={() => setShowAudit((value) => !value)}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ListRestart />
        </Button>
        <Button
          aria-label="Open in system browser"
          onClick={() => {
            void openUrl(currentUrl).catch(() =>
              toast.error("Failed to open link"),
            );
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ExternalLink />
        </Button>
      </form>
      <div className="relative min-h-0 flex-1 bg-background" ref={contentRef}>
        {mode === "fallback" ? (
          <>
            <iframe
              className="h-full w-full border-0 bg-background"
              data-testid="workspace-browser-frame"
              key={`${currentUrl}:${fallback.reloadKey}`}
              referrerPolicy="no-referrer"
              sandbox="allow-forms allow-popups allow-scripts"
              src={currentUrl}
              title={resource.title ?? `Buzz browser: ${currentUrl}`}
            />
            <div className="pointer-events-none absolute inset-x-3 bottom-3 rounded-lg bg-background/90 px-3 py-2 text-xs text-muted-foreground shadow-sm backdrop-blur">
              <p>Some sites block embedded browsing.</p>
              <p>
                Native browsing is unavailable in this surface; use the
                external-browser button when needed.
              </p>
            </div>
          </>
        ) : (
          <div
            className="flex h-full items-center justify-center gap-2 text-sm text-muted-foreground"
            data-testid="workspace-native-browser-host"
          >
            {mode === "connecting" ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                Starting isolated ASV Buzz browser…
              </>
            ) : (
              <span className="sr-only">Native browser active</span>
            )}
          </div>
        )}
        {showAudit ? (
          <div
            className="absolute inset-x-3 bottom-3 z-10 max-h-[55%] overflow-auto rounded-xl border bg-background/95 p-3 shadow-xl backdrop-blur"
            data-testid="workspace-browser-audit"
          >
            <div className="mb-2 flex items-center justify-between">
              <div>
                <p className="text-xs font-semibold">Browser action stream</p>
                <p className="text-3xs text-muted-foreground">
                  Persistent ASV Buzz audit; no credentials are recorded.
                </p>
              </div>
              <Button
                aria-label="Close browser action history"
                onClick={() => setShowAudit(false)}
                size="icon-xs"
                type="button"
                variant="ghost"
              >
                <X />
              </Button>
            </div>
            <div className="mb-3 flex flex-wrap gap-1.5">
              <Button
                disabled={mode !== "native" || extracting}
                onClick={() => {
                  setExtracting(true);
                  void invokeTauri<BrowserPageExtraction>(
                    "extract_workspace_browser_page",
                    { actor: null },
                  )
                    .then((result) =>
                      toast.success(
                        `Extracted ${result.text.length.toLocaleString()} characters`,
                      ),
                    )
                    .catch((error: unknown) =>
                      toast.error(
                        error instanceof Error
                          ? error.message
                          : "Extraction failed",
                      ),
                    )
                    .finally(() => setExtracting(false));
                }}
                size="xs"
                type="button"
                variant="outline"
              >
                <FileSearch />
                Extract page
              </Button>
              <Button
                disabled={mode !== "native"}
                onClick={() => {
                  void invokeTauri<{ completionState: string; reason: string }>(
                    "capture_workspace_browser",
                    { actor: null },
                  ).then((result) => {
                    if (result.completionState !== "completed") {
                      toast.info("Screenshot unavailable", {
                        description: result.reason,
                      });
                    }
                  });
                }}
                size="xs"
                type="button"
                variant="outline"
              >
                <Camera />
                Screenshot
              </Button>
              <Button
                disabled={mode !== "native"}
                onClick={() =>
                  void invokeTauri("clear_workspace_browser_session").then(() =>
                    toast.success("Browser session state cleared"),
                  )
                }
                size="xs"
                type="button"
                variant="outline"
              >
                Clear session
              </Button>
              <Button
                disabled={mode !== "native"}
                onClick={() =>
                  void invokeTauri("clear_workspace_browser_profile").then(() =>
                    toast.success("ASV Buzz browser profile cleared"),
                  )
                }
                size="xs"
                type="button"
                variant="destructive"
              >
                <Trash2 />
                Clear profile
              </Button>
            </div>
            <ol className="space-y-1.5">
              {[...(nativeState?.actions ?? [])].reverse().map((action) => (
                <li
                  className="rounded-lg bg-muted/60 px-2.5 py-2 text-3xs"
                  key={action.id}
                >
                  <div className="flex justify-between gap-3">
                    <span className="font-medium">
                      {action.actor} · {action.action}
                    </span>
                    <span className="text-muted-foreground">
                      {action.outcome}
                    </span>
                  </div>
                  {action.target ? (
                    <p className="mt-0.5 truncate text-muted-foreground">
                      {action.target}
                    </p>
                  ) : null}
                </li>
              ))}
            </ol>
          </div>
        ) : null}
      </div>
    </div>
  );
}

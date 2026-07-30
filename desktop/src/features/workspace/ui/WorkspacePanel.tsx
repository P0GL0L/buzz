import * as React from "react";
import {
  ArrowLeft,
  ArrowRight,
  Download,
  ExternalLink,
  FileQuestion,
  Globe2,
  Loader2,
  RefreshCw,
  X,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

import { invokeTauri } from "@/shared/api/tauri";
import { fetchMediaBytes } from "@/shared/api/tauriMedia";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/button";
import { Markdown } from "@/shared/ui/markdown";
import { OverlayPanelBackdrop } from "@/shared/ui/OverlayPanelBackdrop";
import {
  classifyArtifactPreview,
  formatWorkspaceFileSize,
  normalizeBrowserUrl,
  type ArtifactWorkspaceResource,
  type BrowserWorkspaceResource,
  type WorkspaceResource,
} from "../lib/workspaceResource";
import { OfficeArtifactReader } from "./OfficeArtifactReader";

type ArtifactLoadState =
  | { phase: "idle" }
  | { phase: "loading" }
  | { phase: "error"; message: string }
  | { phase: "ready"; blobUrl?: string; text?: string };

function ArtifactReader({ resource }: { resource: ArtifactWorkspaceResource }) {
  const previewKind = classifyArtifactPreview(resource);
  if (
    previewKind === "docx" ||
    previewKind === "xlsx" ||
    previewKind === "pptx"
  ) {
    return <OfficeArtifactReader resource={resource} />;
  }
  return <BasicArtifactReader previewKind={previewKind} resource={resource} />;
}

function BasicArtifactReader({
  previewKind,
  resource,
}: {
  previewKind: ReturnType<typeof classifyArtifactPreview>;
  resource: ArtifactWorkspaceResource;
}) {
  const imageMime =
    previewKind === "image" && resource.mime?.startsWith("image/")
      ? resource.mime
      : "application/octet-stream";
  const [state, setState] = React.useState<ArtifactLoadState>({
    phase: previewKind === "unsupported" ? "idle" : "loading",
  });

  React.useEffect(() => {
    if (previewKind === "unsupported") {
      setState({ phase: "idle" });
      return;
    }

    let active = true;
    let blobUrl: string | undefined;
    setState({ phase: "loading" });

    void fetchMediaBytes(resource.url)
      .then((bytes) => {
        if (!active) return;

        if (previewKind === "pdf") {
          const signature = new TextDecoder("ascii").decode(bytes.slice(0, 5));
          if (signature !== "%PDF-") {
            throw new Error("This file does not contain a valid PDF header.");
          }
          const blob = new Blob([bytes], { type: "application/pdf" });
          blobUrl = URL.createObjectURL(blob);
          setState({ phase: "ready", blobUrl });
          return;
        }

        if (previewKind === "image") {
          const blob = new Blob([bytes], { type: imageMime });
          blobUrl = URL.createObjectURL(blob);
          setState({ phase: "ready", blobUrl });
          return;
        }

        const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
        setState({ phase: "ready", text });
      })
      .catch((error: unknown) => {
        if (!active) return;
        setState({
          phase: "error",
          message:
            error instanceof Error ? error.message : "Couldn’t load this file.",
        });
      });

    return () => {
      active = false;
      if (blobUrl) URL.revokeObjectURL(blobUrl);
    };
  }, [imageMime, previewKind, resource.url]);

  if (previewKind === "unsupported") {
    return (
      <div
        className="flex h-full flex-col items-center justify-center gap-3 px-8 text-center"
        data-testid="workspace-artifact-unsupported"
      >
        <span className="flex h-12 w-12 items-center justify-center rounded-2xl bg-muted text-muted-foreground">
          <FileQuestion className="h-5 w-5" />
        </span>
        <div>
          <h2 className="text-base font-semibold">Preview not available yet</h2>
          <p className="mt-1 max-w-sm text-sm text-muted-foreground">
            This reader supports images, PDF, Markdown, JSON, CSV, and plain
            text. Download the original to open this format in its native app.
          </p>
        </div>
      </div>
    );
  }

  if (state.phase === "loading" || state.phase === "idle") {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        Loading preview…
      </div>
    );
  }

  if (state.phase === "error") {
    return (
      <div
        className="flex h-full flex-col items-center justify-center gap-2 px-8 text-center"
        data-testid="workspace-artifact-error"
      >
        <h2 className="text-base font-semibold">Couldn’t open this preview</h2>
        <p className="max-w-sm text-sm text-muted-foreground">
          {state.message}
        </p>
      </div>
    );
  }

  if (previewKind === "pdf" && state.blobUrl) {
    return (
      <iframe
        className="h-full w-full border-0 bg-background"
        data-testid="workspace-pdf-preview"
        src={state.blobUrl}
        title={`Preview of ${resource.filename}`}
      />
    );
  }

  if (previewKind === "image" && state.blobUrl) {
    return (
      <div
        className="flex h-full items-center justify-center overflow-auto bg-muted/20 p-5"
        data-testid="workspace-image-preview"
      >
        <img
          alt={`Preview of ${resource.filename}`}
          className="max-h-full max-w-full object-contain"
          src={state.blobUrl}
        />
      </div>
    );
  }

  if (previewKind === "markdown") {
    return (
      <div
        className="h-full overflow-auto px-6 py-5"
        data-testid="workspace-markdown-preview"
      >
        <Markdown content={state.text ?? ""} interactive={false} />
      </div>
    );
  }

  return (
    <pre
      className="h-full overflow-auto whitespace-pre-wrap break-words p-5 font-mono text-sm leading-6 text-foreground"
      data-testid="workspace-text-preview"
    >
      {state.text ?? ""}
    </pre>
  );
}

function BrowserReader({ resource }: { resource: BrowserWorkspaceResource }) {
  const initialUrl =
    normalizeBrowserUrl(resource.url) ?? "https://example.com/";
  const [history, setHistory] = React.useState([initialUrl]);
  const [historyIndex, setHistoryIndex] = React.useState(0);
  const [input, setInput] = React.useState(initialUrl);
  const [reloadKey, setReloadKey] = React.useState(0);
  const currentUrl = history[historyIndex] ?? initialUrl;

  React.useEffect(() => {
    const nextUrl = normalizeBrowserUrl(resource.url);
    if (!nextUrl) return;
    setHistory([nextUrl]);
    setHistoryIndex(0);
    setInput(nextUrl);
  }, [resource.url]);

  function navigate(value: string) {
    const normalized = normalizeBrowserUrl(value);
    if (!normalized) {
      toast.error("Enter an HTTP or HTTPS address");
      return;
    }
    setHistory((current) => [
      ...current.slice(0, historyIndex + 1),
      normalized,
    ]);
    setHistoryIndex((current) => current + 1);
    setInput(normalized);
  }

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
          disabled={historyIndex === 0}
          onClick={() => {
            const nextIndex = Math.max(0, historyIndex - 1);
            setHistoryIndex(nextIndex);
            setInput(history[nextIndex] ?? input);
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ArrowLeft />
        </Button>
        <Button
          aria-label="Forward"
          disabled={historyIndex >= history.length - 1}
          onClick={() => {
            const nextIndex = Math.min(history.length - 1, historyIndex + 1);
            setHistoryIndex(nextIndex);
            setInput(history[nextIndex] ?? input);
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ArrowRight />
        </Button>
        <Button
          aria-label="Reload"
          onClick={() => setReloadKey((current) => current + 1)}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <RefreshCw />
        </Button>
        <input
          aria-label="Browser address"
          className="h-8 min-w-0 flex-1 rounded-lg border border-input/50 bg-muted/35 px-3 text-sm outline-hidden focus:border-ring focus:ring-1 focus:ring-ring"
          onChange={(event) => setInput(event.target.value)}
          spellCheck={false}
          value={input}
        />
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
      <div className="relative min-h-0 flex-1 bg-background">
        <iframe
          className="h-full w-full border-0 bg-background"
          data-testid="workspace-browser-frame"
          key={`${currentUrl}:${reloadKey}`}
          referrerPolicy="no-referrer"
          sandbox="allow-forms allow-popups allow-scripts"
          src={currentUrl}
          title={resource.title ?? `Buzz browser: ${currentUrl}`}
        />
        <p className="pointer-events-none absolute inset-x-3 bottom-3 rounded-lg bg-background/90 px-3 py-2 text-xs text-muted-foreground shadow-sm backdrop-blur">
          Some sites block embedded browsing. Use the external-browser button
          when a page refuses to load.
        </p>
      </div>
    </div>
  );
}

export function WorkspacePanel({
  onClose,
  resource,
}: {
  onClose: () => void;
  resource: WorkspaceResource;
}) {
  const isArtifact = resource.kind === "artifact";
  const normalizedBrowserUrl =
    resource.kind === "browser" ? normalizeBrowserUrl(resource.url) : null;
  const title = isArtifact
    ? resource.filename
    : resource.title ||
      (normalizedBrowserUrl
        ? new URL(normalizedBrowserUrl).hostname
        : "Buzz browser");
  const metadata = isArtifact
    ? [resource.mime, formatWorkspaceFileSize(resource.size)]
        .filter(Boolean)
        .join(" · ")
    : resource.url;

  React.useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onClose();
    }
    window.addEventListener("keydown", handleKeyDown, { capture: true });
    return () =>
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
  }, [onClose]);

  return (
    <>
      <OverlayPanelBackdrop onClose={onClose} />
      <aside
        aria-label={isArtifact ? "Artifact reader" : "Buzz browser"}
        className={cn(
          "fixed inset-y-0 right-0 z-50 flex w-full max-w-3xl flex-col border-l border-border bg-background shadow-2xl",
          "animate-in slide-in-from-right duration-200",
        )}
        data-testid="workspace-panel"
      >
        <header className="flex min-h-14 shrink-0 items-center gap-3 border-b border-border/70 px-4 py-2">
          <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-muted text-muted-foreground">
            {isArtifact ? <FileQuestion /> : <Globe2 />}
          </span>
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-sm font-semibold" title={title}>
              {title}
            </h1>
            <p
              className="truncate text-xs text-muted-foreground"
              title={metadata}
            >
              {metadata}
            </p>
          </div>
          {isArtifact ? (
            <Button
              aria-label={`Download ${resource.filename}`}
              data-testid="workspace-download"
              onClick={() => {
                invokeTauri("download_file", {
                  url: resource.url,
                  filename: resource.filename,
                }).catch((error: unknown) => {
                  toast.error(
                    error instanceof Error ? error.message : "Download failed",
                  );
                });
              }}
              size="icon"
              type="button"
              variant="ghost"
            >
              <Download />
            </Button>
          ) : null}
          <Button
            aria-label="Close workspace"
            onClick={onClose}
            size="icon"
            type="button"
            variant="ghost"
          >
            <X />
          </Button>
        </header>
        <div className="min-h-0 flex-1">
          {resource.kind === "artifact" ? (
            <ArtifactReader resource={resource} />
          ) : (
            <BrowserReader resource={resource} />
          )}
        </div>
      </aside>
    </>
  );
}

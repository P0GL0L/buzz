import * as React from "react";
import {
  Download,
  ExternalLink,
  FileQuestion,
  Globe2,
  History,
  Info,
  Loader2,
  X,
} from "lucide-react";
import { openPath } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";

import { invokeTauri } from "@/shared/api/tauri";
import { fetchMediaBytes } from "@/shared/api/tauriMedia";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/button";
import { Markdown } from "@/shared/ui/markdown";
import { useWorkspacePanelWidth } from "../useWorkspacePanelWidth";
import {
  classifyArtifactPreview,
  formatWorkspaceFileSize,
  normalizeBrowserUrl,
  resolvedArtifactId,
  type ArtifactWorkspaceResource,
  type WorkspaceResource,
} from "../lib/workspaceResource";
import { NativeBrowserReader } from "./NativeBrowserReader";
import { OfficeArtifactReader } from "./OfficeArtifactReader";
import { PdfArtifactReader } from "./PdfArtifactReader";
import { requestOpenWorkspaceResource } from "../openWorkspaceResourceEvent";

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
  if (previewKind === "pdf") {
    return (
      <div className="h-full min-h-0" data-testid="workspace-pdf-preview">
        <PdfArtifactReader resource={resource} />
      </div>
    );
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
  const [textMode, setTextMode] = React.useState<"rendered" | "source">(
    "rendered",
  );

  React.useEffect(() => {
    if (previewKind === "unsupported") {
      setState({ phase: "idle" });
      return;
    }

    let active = true;
    let blobUrl: string | undefined;
    setState({ phase: "loading" });

    void (
      resource.localPath
        ? invokeTauri<ArrayBuffer>("fetch_workspace_browser_download", {
            path: resource.localPath,
          })
        : fetchMediaBytes(resource.url)
    )
      .then((bytes) => {
        if (!active) return;

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
  }, [imageMime, previewKind, resource.localPath, resource.url]);

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
      <div className="flex h-full min-h-0 flex-col">
        <div className="flex shrink-0 justify-end gap-1 border-b px-3 py-2">
          <Button
            onClick={() => setTextMode("rendered")}
            size="xs"
            type="button"
            variant={textMode === "rendered" ? "secondary" : "ghost"}
          >
            Rendered
          </Button>
          <Button
            onClick={() => setTextMode("source")}
            size="xs"
            type="button"
            variant={textMode === "source" ? "secondary" : "ghost"}
          >
            Source
          </Button>
        </div>
        {textMode === "rendered" ? (
          <div
            className="min-h-0 flex-1 overflow-auto px-6 py-5"
            data-testid="workspace-markdown-preview"
          >
            <Markdown content={state.text ?? ""} interactive={false} />
          </div>
        ) : (
          <pre
            className="min-h-0 flex-1 overflow-auto whitespace-pre-wrap break-words p-5 font-mono text-sm leading-6"
            data-testid="workspace-markdown-source"
          >
            {state.text ?? ""}
          </pre>
        )}
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

export function WorkspacePanel({
  onClose,
  resource,
}: {
  onClose: () => void;
  resource: WorkspaceResource;
}) {
  const isArtifact = resource.kind === "artifact";
  const [showArtifactDetails, setShowArtifactDetails] = React.useState(false);
  const panelWidth = useWorkspacePanelWidth();
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
    <aside
      aria-label={isArtifact ? "Artifact reader" : "Buzz browser"}
      className={cn(
        "group/workspace-panel relative flex h-full min-w-0 shrink-0 flex-row overflow-hidden bg-background",
        "animate-in slide-in-from-right duration-200",
      )}
      data-testid="workspace-panel"
      style={{ maxWidth: panelWidth.maxWidth, width: panelWidth.widthPx }}
    >
      <button
        aria-label="Resize workspace"
        className="group/workspace-resize relative h-full w-2 shrink-0 touch-none cursor-col-resize border-0 bg-transparent p-0"
        data-testid="workspace-panel-resize-handle"
        onDoubleClick={
          panelWidth.canReset ? panelWidth.onResetWidth : undefined
        }
        onPointerDown={panelWidth.onResizeStart}
        title={`Drag to resize. Current width: ${Math.round(panelWidth.widthPx)}px.${
          panelWidth.canReset ? " Double-click to reset width." : ""
        }`}
        type="button"
      >
        <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-transparent transition-colors group-hover/workspace-resize:bg-border group-focus-visible/workspace-resize:bg-border" />
      </button>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border-l border-border">
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
          {isArtifact && resource.localPath ? (
            <Button
              aria-label={`Open ${resource.filename} in its native application`}
              onClick={() => {
                void openPath(resource.localPath ?? "").catch(
                  (error: unknown) =>
                    toast.error(
                      error instanceof Error
                        ? error.message
                        : "Couldn’t open the native application",
                    ),
                );
              }}
              size="icon"
              type="button"
              variant="ghost"
            >
              <ExternalLink />
            </Button>
          ) : null}
          {isArtifact && !resource.localPath ? (
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
          {isArtifact ? (
            <Button
              aria-label="Artifact metadata and revision history"
              aria-pressed={showArtifactDetails}
              onClick={() => setShowArtifactDetails((visible) => !visible)}
              size="icon"
              type="button"
              variant={showArtifactDetails ? "secondary" : "ghost"}
            >
              {resource.revisions?.length ? <History /> : <Info />}
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
        {isArtifact && showArtifactDetails ? (
          <section
            aria-label="Artifact metadata"
            className="max-h-52 shrink-0 overflow-auto border-b bg-muted/20 px-4 py-3 text-xs"
            data-testid="workspace-artifact-metadata"
          >
            <dl className="grid gap-x-4 gap-y-1 sm:grid-cols-[8rem_minmax(0,1fr)]">
              <dt className="text-muted-foreground">Artifact</dt>
              <dd
                className="truncate font-mono"
                title={resolvedArtifactId(resource)}
              >
                {resolvedArtifactId(resource)}
              </dd>
              <dt className="text-muted-foreground">Version</dt>
              <dd>{resource.version ?? 1}</dd>
              <dt className="text-muted-foreground">Source</dt>
              <dd>{resource.source ?? "attachment"}</dd>
              {resource.sha256 ? (
                <>
                  <dt className="text-muted-foreground">SHA-256</dt>
                  <dd className="truncate font-mono" title={resource.sha256}>
                    {resource.sha256}
                  </dd>
                </>
              ) : null}
              {resource.signer ? (
                <>
                  <dt className="text-muted-foreground">Signer</dt>
                  <dd className="truncate font-mono" title={resource.signer}>
                    {resource.signer}
                  </dd>
                </>
              ) : null}
              {resource.correlationId ? (
                <>
                  <dt className="text-muted-foreground">Task correlation</dt>
                  <dd className="truncate font-mono">
                    {resource.correlationId}
                  </dd>
                </>
              ) : null}
            </dl>
            {resource.revisions?.length ? (
              <div className="mt-3 border-t pt-3">
                <p className="mb-2 font-medium">Signed revisions</p>
                <ol className="space-y-1">
                  {[...resource.revisions]
                    .sort((a, b) => b.version - a.version)
                    .map((revision) => (
                      <li key={revision.eventId}>
                        <button
                          className="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left hover:bg-muted"
                          onClick={() =>
                            requestOpenWorkspaceResource({
                              kind: "artifact",
                              artifactId: resolvedArtifactId(resource),
                              correlationId: resource.correlationId,
                              filename: revision.filename,
                              mime: revision.mime,
                              parentEventId: revision.parentEventId,
                              revisions: resource.revisions,
                              sha256: revision.sha256,
                              signer: revision.signer,
                              size: revision.size,
                              source: revision.source,
                              threadId: resource.threadId,
                              url: revision.url,
                              version: revision.version,
                            })
                          }
                          type="button"
                        >
                          <span>Version {revision.version}</span>
                          <span className="text-muted-foreground">
                            {revision.filename}
                          </span>
                        </button>
                      </li>
                    ))}
                </ol>
              </div>
            ) : (
              <p className="mt-3 border-t pt-3 text-muted-foreground">
                No additional signed revisions are attached to this thread.
              </p>
            )}
          </section>
        ) : null}
        <div className="min-h-0 flex-1">
          {resource.kind === "artifact" ? (
            <ArtifactReader resource={resource} />
          ) : (
            <NativeBrowserReader resource={resource} />
          )}
        </div>
      </div>
    </aside>
  );
}

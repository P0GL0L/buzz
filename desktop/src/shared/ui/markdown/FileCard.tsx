import * as React from "react";
import { Download, Eye, FileText } from "lucide-react";
import { toast } from "sonner";

import { requestOpenWorkspaceResource } from "@/features/workspace/openWorkspaceResourceEvent";
import { invokeTauri } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";
import { useSmoothCorners } from "@/shared/ui/smoothCorners";
import { formatWorkspaceFileSize } from "@/features/workspace/lib/workspaceResource";

/**
 * File card for a generic (non-image, non-video) attachment: icon, filename,
 * size, an in-app reader action, and a separate download action.
 *
 * Downloads go through the native `download_file` Tauri command (HTTP inside
 * the app's tunnel + a save dialog), not a plain `<a download>` link. A bare
 * link navigates the webview to the blob URL, which escapes to the OS browser
 * and gets bounced to a corporate CDN interstitial ("browser not supported").
 * The native command mirrors the image-download path.
 */
export function FileCard({
  href,
  filename,
  mime,
  size,
}: {
  href: string;
  filename: string;
  mime?: string;
  size?: number;
}) {
  const cardRef = React.useRef<HTMLDivElement | null>(null);
  const sizeLabel = formatWorkspaceFileSize(size);
  useSmoothCorners(cardRef);

  return (
    <div
      ref={cardRef}
      data-testid="file-card"
      className="my-1 inline-flex max-w-sm items-center gap-3 rounded-2xl border border-border/70 bg-muted/40 px-3 py-2 text-left no-underline transition-colors hover:bg-muted/70"
      style={{ borderRadius: "1rem" }}
    >
      <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-background text-muted-foreground">
        <FileText className="h-4 w-4" />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium text-foreground">
          {filename}
        </span>
        {sizeLabel ? (
          <span className="block text-xs text-muted-foreground">
            {sizeLabel}
          </span>
        ) : null}
      </span>
      <span className="flex shrink-0 items-center gap-1">
        <Button
          aria-label={`Open ${filename} in workspace`}
          data-testid="file-card-preview"
          onClick={() =>
            requestOpenWorkspaceResource({
              kind: "artifact",
              url: href,
              filename,
              mime,
              size,
            })
          }
          size="icon-xs"
          title="Open in workspace"
          type="button"
          variant="ghost"
        >
          <Eye />
        </Button>
        <Button
          aria-label={`Download ${filename}`}
          data-testid="file-card-download"
          onClick={() => {
            invokeTauri("download_file", { url: href, filename }).catch(
              (err: unknown) => {
                const msg =
                  err instanceof Error ? err.message : "Download failed";
                toast.error(msg);
              },
            );
          }}
          size="icon-xs"
          title="Download"
          type="button"
          variant="ghost"
        >
          <Download />
        </Button>
      </span>
    </div>
  );
}

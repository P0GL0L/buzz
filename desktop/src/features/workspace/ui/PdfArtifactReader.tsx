import * as React from "react";
import {
  ChevronLeft,
  ChevronRight,
  Copy,
  Loader2,
  Search,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import {
  getDocument,
  GlobalWorkerOptions,
  TextLayer,
  type PDFDocumentProxy,
  type PDFPageProxy,
} from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.mjs?url";
import "pdfjs-dist/web/pdf_viewer.css";
import { toast } from "sonner";

import { invokeTauri } from "@/shared/api/tauri";
import { fetchMediaBytes } from "@/shared/api/tauriMedia";
import { Button } from "@/shared/ui/button";
import type { ArtifactWorkspaceResource } from "../lib/workspaceResource";

GlobalWorkerOptions.workerSrc = workerUrl;

const MAX_PDF_BYTES = 50 * 1024 * 1024;
const MAX_SEARCH_PAGES = 500;
const MAX_SEARCH_CHARACTERS = 2_000_000;
const MAX_THUMBNAILS = 200;

type LoadState =
  | { phase: "loading" }
  | { phase: "error"; message: string }
  | { phase: "ready"; document: PDFDocumentProxy };

function boundedPage(value: number, total: number): number {
  return Math.max(1, Math.min(total, Math.round(value) || 1));
}

function pageText(items: Awaited<ReturnType<PDFPageProxy["getTextContent"]>>) {
  return items.items
    .map((item) => ("str" in item ? item.str : ""))
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}

function PdfPage({
  document,
  pageNumber,
  zoom,
}: {
  document: PDFDocumentProxy;
  pageNumber: number;
  zoom: number;
}) {
  const canvasRef = React.useRef<HTMLCanvasElement>(null);
  const textLayerRef = React.useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = React.useState({ height: 0, width: 0 });

  React.useEffect(() => {
    let cancelled = false;
    let renderTask: ReturnType<PDFPageProxy["render"]> | null = null;
    let textLayer: TextLayer | null = null;
    void document.getPage(pageNumber).then(async (page) => {
      if (cancelled) return;
      const viewport = page.getViewport({ scale: zoom });
      const canvas = canvasRef.current;
      const layer = textLayerRef.current;
      if (!canvas || !layer) return;
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.floor(viewport.width * ratio);
      canvas.height = Math.floor(viewport.height * ratio);
      canvas.style.width = `${viewport.width}px`;
      canvas.style.height = `${viewport.height}px`;
      layer.replaceChildren();
      layer.style.width = `${viewport.width}px`;
      layer.style.height = `${viewport.height}px`;
      layer.style.setProperty("--total-scale-factor", String(zoom));
      setDimensions({ height: viewport.height, width: viewport.width });

      const context = canvas.getContext("2d", { alpha: false });
      if (!context) throw new Error("PDF canvas is unavailable");
      renderTask = page.render({
        canvas,
        canvasContext: context,
        transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0],
        viewport,
      });
      const textContent = await page.getTextContent();
      if (cancelled) return;
      textLayer = new TextLayer({
        container: layer,
        textContentSource: textContent,
        viewport,
      });
      await Promise.all([renderTask.promise, textLayer.render()]);
    });
    return () => {
      cancelled = true;
      renderTask?.cancel();
      textLayer?.cancel();
    };
  }, [document, pageNumber, zoom]);

  return (
    <div
      className="relative bg-white shadow-lg"
      data-testid="workspace-pdf-page"
      style={dimensions}
    >
      <canvas aria-label={`PDF page ${pageNumber}`} ref={canvasRef} />
      <div className="textLayer" ref={textLayerRef} />
    </div>
  );
}

function PdfThumbnail({
  active,
  document,
  onSelect,
  pageNumber,
}: {
  active: boolean;
  document: PDFDocumentProxy;
  onSelect: () => void;
  pageNumber: number;
}) {
  const canvasRef = React.useRef<HTMLCanvasElement>(null);
  React.useEffect(() => {
    let cancelled = false;
    let task: ReturnType<PDFPageProxy["render"]> | null = null;
    void document.getPage(pageNumber).then((page) => {
      if (cancelled || !canvasRef.current) return;
      const viewport = page.getViewport({ scale: 0.18 });
      const canvas = canvasRef.current;
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      const context = canvas.getContext("2d", { alpha: false });
      if (!context) return;
      task = page.render({ canvas, canvasContext: context, viewport });
      return task.promise;
    });
    return () => {
      cancelled = true;
      task?.cancel();
    };
  }, [document, pageNumber]);

  return (
    <button
      aria-current={active ? "page" : undefined}
      aria-label={`Go to PDF page ${pageNumber}`}
      className={
        active
          ? "rounded-lg border border-primary bg-primary/10 p-1.5"
          : "rounded-lg border border-transparent p-1.5 hover:border-border"
      }
      onClick={onSelect}
      type="button"
    >
      <canvas className="mx-auto max-w-full bg-white" ref={canvasRef} />
      <span className="mt-1 block text-center text-3xs text-muted-foreground">
        {pageNumber}
      </span>
    </button>
  );
}

export function PdfArtifactReader({
  resource,
}: {
  resource: ArtifactWorkspaceResource;
}) {
  const [state, setState] = React.useState<LoadState>({ phase: "loading" });
  const [pageNumber, setPageNumber] = React.useState(1);
  const [zoom, setZoom] = React.useState(1);
  const [query, setQuery] = React.useState("");
  const [matches, setMatches] = React.useState<number[]>([]);
  const [searching, setSearching] = React.useState(false);

  React.useEffect(() => {
    let active = true;
    let document: PDFDocumentProxy | null = null;
    const loadingTaskPromise = (
      resource.localPath
        ? invokeTauri<ArrayBuffer>("fetch_workspace_browser_download", {
            path: resource.localPath,
          })
        : fetchMediaBytes(resource.url)
    )
      .then((bytes) => {
        if (bytes.byteLength > MAX_PDF_BYTES) {
          throw new Error("PDF exceeds the 50 MiB workspace limit.");
        }
        const signature = new TextDecoder("ascii").decode(bytes.slice(0, 5));
        if (signature !== "%PDF-") {
          throw new Error("This file does not contain a valid PDF header.");
        }
        return getDocument({
          data: new Uint8Array(bytes),
          useWorkerFetch: false,
        }).promise;
      })
      .then((loaded) => {
        document = loaded;
        if (active) setState({ phase: "ready", document: loaded });
      })
      .catch((cause: unknown) => {
        if (!active) return;
        const message =
          cause instanceof Error ? cause.message : "Couldn’t open this PDF.";
        setState({
          phase: "error",
          message: /password/i.test(message)
            ? "Encrypted or password-protected PDFs are not previewed."
            : message,
        });
      });
    return () => {
      active = false;
      void loadingTaskPromise.finally(() => document?.destroy());
    };
  }, [resource.localPath, resource.url]);

  const runSearch = React.useCallback(async () => {
    if (state.phase !== "ready") return;
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) {
      setMatches([]);
      return;
    }
    setSearching(true);
    try {
      const found: number[] = [];
      let searchedCharacters = 0;
      const pageLimit = Math.min(state.document.numPages, MAX_SEARCH_PAGES);
      for (let page = 1; page <= pageLimit; page += 1) {
        const proxy = await state.document.getPage(page);
        const text = pageText(await proxy.getTextContent());
        searchedCharacters += text.length;
        if (text.toLocaleLowerCase().includes(needle)) found.push(page);
        if (searchedCharacters >= MAX_SEARCH_CHARACTERS) break;
      }
      setMatches(found);
      if (found[0]) setPageNumber(found[0]);
    } finally {
      setSearching(false);
    }
  }, [query, state]);

  React.useEffect(() => {
    if (state.phase !== "ready") return;
    const totalPages = state.document.numPages;
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }
      if (event.key === "PageUp" || event.key === "ArrowLeft") {
        event.preventDefault();
        setPageNumber((current) => boundedPage(current - 1, totalPages));
      } else if (event.key === "PageDown" || event.key === "ArrowRight") {
        event.preventDefault();
        setPageNumber((current) => boundedPage(current + 1, totalPages));
      } else if ((event.metaKey || event.ctrlKey) && event.key === "+") {
        event.preventDefault();
        setZoom((current) => Math.min(2.5, current + 0.15));
      } else if ((event.metaKey || event.ctrlKey) && event.key === "-") {
        event.preventDefault();
        setZoom((current) => Math.max(0.5, current - 0.15));
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [state]);

  if (state.phase === "loading") {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        Loading PDF…
      </div>
    );
  }
  if (state.phase === "error") {
    return (
      <div
        className="flex h-full items-center justify-center px-8 text-center text-sm text-destructive"
        data-testid="workspace-artifact-error"
      >
        {state.message}
      </div>
    );
  }

  const totalPages = state.document.numPages;
  return (
    <div
      className="flex h-full min-h-0 flex-col"
      data-testid="workspace-pdf-reader"
    >
      <form
        className="flex shrink-0 flex-wrap items-center gap-2 border-b px-3 py-2"
        onSubmit={(event) => {
          event.preventDefault();
          void runSearch();
        }}
      >
        <Button
          aria-label="Previous PDF page"
          disabled={pageNumber <= 1}
          onClick={() =>
            setPageNumber((current) => boundedPage(current - 1, totalPages))
          }
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ChevronLeft />
        </Button>
        <label className="flex items-center gap-1 text-xs">
          <span className="sr-only">PDF page</span>
          <input
            className="h-7 w-14 rounded-md border bg-background px-2 text-center"
            min={1}
            onChange={(event) =>
              setPageNumber(boundedPage(Number(event.target.value), totalPages))
            }
            type="number"
            value={pageNumber}
          />
          <span className="text-muted-foreground">of {totalPages}</span>
        </label>
        <Button
          aria-label="Next PDF page"
          disabled={pageNumber >= totalPages}
          onClick={() =>
            setPageNumber((current) => boundedPage(current + 1, totalPages))
          }
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ChevronRight />
        </Button>
        <span className="mx-1 h-5 w-px bg-border" />
        <Button
          aria-label="Zoom out"
          onClick={() => setZoom((current) => Math.max(0.5, current - 0.15))}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ZoomOut />
        </Button>
        <span className="w-10 text-center text-xs text-muted-foreground">
          {Math.round(zoom * 100)}%
        </span>
        <Button
          aria-label="Zoom in"
          onClick={() => setZoom((current) => Math.min(2.5, current + 0.15))}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ZoomIn />
        </Button>
        <label className="relative ml-auto min-w-44 flex-1 sm:max-w-64">
          <span className="sr-only">Search PDF</span>
          <Search className="pointer-events-none absolute left-2.5 top-2 h-3.5 w-3.5 text-muted-foreground" />
          <input
            className="h-8 w-full rounded-lg border bg-background pl-8 pr-2 text-xs"
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search document"
            type="search"
            value={query}
          />
        </label>
        <Button disabled={searching} size="xs" type="submit" variant="outline">
          {searching ? <Loader2 className="animate-spin" /> : "Find"}
        </Button>
        <Button
          aria-label="Copy selected PDF text"
          onClick={() => {
            const selected = window.getSelection()?.toString().trim();
            if (!selected) {
              toast.info("Select text in the PDF first");
              return;
            }
            void navigator.clipboard.writeText(selected);
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <Copy />
        </Button>
        {query.trim() ? (
          <span className="text-3xs text-muted-foreground">
            {matches.length} matching {matches.length === 1 ? "page" : "pages"}
          </span>
        ) : null}
      </form>
      <div className="flex min-h-0 flex-1">
        <aside
          aria-label="PDF page thumbnails"
          className="hidden w-28 shrink-0 overflow-y-auto border-r bg-muted/20 p-2 sm:block"
        >
          {Array.from(
            { length: Math.min(totalPages, MAX_THUMBNAILS) },
            (_, index) => index + 1,
          ).map((page) => (
            <PdfThumbnail
              active={page === pageNumber}
              document={state.document}
              key={page}
              onSelect={() => setPageNumber(page)}
              pageNumber={page}
            />
          ))}
          {totalPages > MAX_THUMBNAILS ? (
            <p className="px-1 py-3 text-center text-3xs text-muted-foreground">
              Thumbnails limited to the first {MAX_THUMBNAILS} pages.
            </p>
          ) : null}
        </aside>
        <main className="min-h-0 flex-1 overflow-auto bg-muted/35 p-6">
          <div className="mx-auto w-max">
            <PdfPage
              document={state.document}
              pageNumber={pageNumber}
              zoom={zoom}
            />
          </div>
        </main>
      </div>
    </div>
  );
}

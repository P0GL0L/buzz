import * as React from "react";
import {
  ChevronLeft,
  ChevronRight,
  Clipboard,
  FileText,
  Loader2,
  Search,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { toast } from "sonner";

import { invokeTauri } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";
import {
  searchableOfficeText,
  type DocxBlock,
  type OfficePreview,
  type SlidePreview,
  type WorksheetPreview,
} from "../lib/officePreview";
import type { ArtifactWorkspaceResource } from "../lib/workspaceResource";

type LoadState =
  | { phase: "loading" }
  | { phase: "error"; message: string }
  | { phase: "ready"; preview: OfficePreview };

function DocxView({ blocks, query }: { blocks: DocxBlock[]; query: string }) {
  const normalized = query.trim().toLowerCase();
  const visible = normalized
    ? blocks.filter((block) => {
        const text =
          block.kind === "table" ? block.rows.flat().join(" ") : block.text;
        return text.toLowerCase().includes(normalized);
      })
    : blocks;
  return (
    <article className="mx-auto max-w-3xl space-y-3 p-6">
      {visible.map((block, index) => {
        const key = `${block.kind}:${index}`;
        if (block.kind === "heading") {
          const Heading =
            `h${block.level}` as keyof React.JSX.IntrinsicElements;
          return (
            <Heading className="font-semibold text-foreground" key={key}>
              {block.text}
            </Heading>
          );
        }
        if (block.kind === "list") {
          return (
            <div className="flex gap-2 text-sm leading-6" key={key}>
              <span aria-hidden="true">•</span>
              <p>{block.text}</p>
            </div>
          );
        }
        if (block.kind === "table") {
          return (
            <div className="overflow-x-auto rounded-lg border" key={key}>
              <table className="w-full border-collapse text-sm">
                <tbody>
                  {block.rows.map((row) => (
                    <tr key={`${key}:row:${row.join("\u0000")}`}>
                      {row.map((cell) => (
                        <td
                          className="border-b border-r px-3 py-2 align-top last:border-r-0"
                          key={`${key}:cell:${cell}`}
                        >
                          {cell}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          );
        }
        return (
          <p className="text-sm leading-6 text-foreground" key={key}>
            {block.text}
          </p>
        );
      })}
      {visible.length === 0 ? (
        <p className="py-10 text-center text-sm text-muted-foreground">
          No document content matches this search.
        </p>
      ) : null}
    </article>
  );
}

function WorksheetView({
  query,
  sheet,
}: {
  query: string;
  sheet: WorksheetPreview;
}) {
  const normalized = query.trim().toLowerCase();
  const rows = normalized
    ? sheet.rows.filter((row) =>
        row.some((cell) =>
          `${cell.reference} ${cell.value} ${cell.formula ?? ""}`
            .toLowerCase()
            .includes(normalized),
        ),
      )
    : sheet.rows;
  return (
    <div className="h-full overflow-auto">
      <table className="min-w-full border-collapse text-xs">
        <tbody>
          {rows.map((row) => (
            <tr
              key={`${sheet.name}:${row.map((cell) => cell.reference).join(":")}`}
            >
              {row.map((cell) => (
                <td
                  className="min-w-28 border-b border-r px-3 py-2 align-top"
                  key={`${sheet.name}:${cell.reference}`}
                  title={cell.formula ? `Formula: ${cell.formula}` : undefined}
                >
                  <span className="mb-1 block font-mono text-[10px] text-muted-foreground">
                    {cell.reference}
                  </span>
                  <span>{cell.value}</span>
                  {cell.formula ? (
                    <code className="mt-1 block text-[10px] text-muted-foreground">
                      ={cell.formula}
                    </code>
                  ) : null}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 ? (
        <p className="py-10 text-center text-sm text-muted-foreground">
          No worksheet rows match this filter.
        </p>
      ) : null}
    </div>
  );
}

function SlideView({ slide }: { slide: SlidePreview }) {
  return (
    <div className="mx-auto my-6 aspect-video w-[min(90%,900px)] overflow-auto rounded-xl border bg-white p-10 text-zinc-900 shadow-sm">
      <h2 className="text-2xl font-semibold">{slide.title}</h2>
      <div className="mt-6 space-y-3">
        {slide.body.map((line) => (
          <p key={`${slide.number}:body:${line}`}>{line}</p>
        ))}
      </div>
      {slide.notes.length > 0 ? (
        <aside className="mt-8 border-t pt-4 text-sm text-zinc-500">
          <p className="mb-1 font-medium">Speaker notes</p>
          {slide.notes.map((note) => (
            <p key={`${slide.number}:note:${note}`}>{note}</p>
          ))}
        </aside>
      ) : null}
    </div>
  );
}

export function OfficeArtifactReader({
  resource,
}: {
  resource: ArtifactWorkspaceResource;
}) {
  const [state, setState] = React.useState<LoadState>({ phase: "loading" });
  const [mode, setMode] = React.useState<"semantic" | "fidelity">("semantic");
  const [query, setQuery] = React.useState("");
  const [activeSheet, setActiveSheet] = React.useState(0);
  const [activeSlide, setActiveSlide] = React.useState(0);
  const [zoom, setZoom] = React.useState(1);

  React.useEffect(() => {
    let active = true;
    setState({ phase: "loading" });
    void invokeTauri<OfficePreview>("preview_office_artifact", {
      url: resource.url,
      filename: resource.filename,
      mime: resource.mime ?? null,
      includeFidelity: true,
    })
      .then((preview) => {
        if (!active) return;
        setState({ phase: "ready", preview });
        if (preview.fidelity) setMode("fidelity");
      })
      .catch((error: unknown) => {
        if (!active) return;
        setState({
          phase: "error",
          message:
            error instanceof Error
              ? error.message
              : "Couldn’t extract this Office package.",
        });
      });
    return () => {
      active = false;
    };
  }, [resource.filename, resource.mime, resource.url]);

  if (state.phase === "loading") {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        Safely extracting Office preview…
      </div>
    );
  }
  if (state.phase === "error") {
    return (
      <div
        className="flex h-full flex-col items-center justify-center gap-3 px-8 text-center"
        data-testid="workspace-artifact-error"
      >
        <FileText className="h-8 w-8 text-muted-foreground" />
        <h2 className="text-base font-semibold">
          Couldn’t open this Office file
        </h2>
        <p className="max-w-md text-sm text-muted-foreground">
          {state.message}
        </p>
        <p className="max-w-md text-xs text-muted-foreground">
          The original remains available to download. Encrypted, macro-bearing,
          malformed, and oversized packages are intentionally rejected.
        </p>
      </div>
    );
  }

  const { preview } = state;
  const document = preview.document;
  const searchableText = searchableOfficeText(preview);
  const sheet =
    document.kind === "xlsx" ? document.sheets[activeSheet] : undefined;
  const slide =
    document.kind === "pptx" ? document.slides[activeSlide] : undefined;

  return (
    <div
      className="flex h-full min-h-0 flex-col"
      data-testid={`workspace-${preview.format}-preview`}
    >
      <div className="flex flex-wrap items-center gap-2 border-b px-3 py-2">
        <div className="flex rounded-lg bg-muted p-0.5 text-xs">
          <button
            className="rounded-md px-2.5 py-1 data-[active=true]:bg-background data-[active=true]:shadow-sm"
            data-active={mode === "semantic"}
            onClick={() => setMode("semantic")}
            type="button"
          >
            Structured
          </button>
          <button
            className="rounded-md px-2.5 py-1 data-[active=true]:bg-background data-[active=true]:shadow-sm disabled:opacity-50"
            data-active={mode === "fidelity"}
            disabled={!preview.fidelity}
            onClick={() => setMode("fidelity")}
            type="button"
          >
            Rendered
          </button>
        </div>
        {mode === "semantic" ? (
          <label className="relative min-w-44 flex-1">
            <Search className="absolute left-2.5 top-2 h-3.5 w-3.5 text-muted-foreground" />
            <input
              aria-label={
                document.kind === "xlsx"
                  ? "Filter worksheet"
                  : "Search document"
              }
              className="h-8 w-full rounded-lg border bg-background pl-8 pr-3 text-xs"
              onChange={(event) => setQuery(event.target.value)}
              placeholder={
                document.kind === "xlsx" ? "Filter rows…" : "Search…"
              }
              value={query}
            />
          </label>
        ) : (
          <span className="flex-1 text-xs text-muted-foreground">
            Sanitized {preview.fidelity?.renderer} preview
          </span>
        )}
        <Button
          aria-label="Copy extracted text"
          onClick={() => {
            invokeTauri("copy_text_to_clipboard", {
              text: searchableText,
              html: null,
            })
              .then(() => toast.success("Extracted text copied"))
              .catch(() => toast.error("Copy failed"));
          }}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <Clipboard />
        </Button>
        <Button
          aria-label="Zoom out"
          disabled={zoom <= 0.7}
          onClick={() => setZoom((value) => Math.max(0.7, value - 0.1))}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ZoomOut />
        </Button>
        <span className="w-10 text-center text-[10px] text-muted-foreground">
          {Math.round(zoom * 100)}%
        </span>
        <Button
          aria-label="Zoom in"
          disabled={zoom >= 1.8}
          onClick={() => setZoom((value) => Math.min(1.8, value + 0.1))}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <ZoomIn />
        </Button>
      </div>
      {preview.truncated || preview.warnings.length > 0 ? (
        <div className="border-b bg-amber-500/10 px-3 py-2 text-xs text-amber-800 dark:text-amber-300">
          {preview.truncated
            ? "Preview limits were reached; the original file is unchanged. "
            : ""}
          {preview.warnings.join(" ")}
        </div>
      ) : null}
      {mode === "semantic" && document.kind === "xlsx" ? (
        <div className="flex gap-1 overflow-x-auto border-b px-3 py-2">
          {document.sheets.map((item, index) => (
            <button
              className="shrink-0 rounded-md px-3 py-1 text-xs data-[active=true]:bg-muted data-[active=true]:font-medium"
              data-active={index === activeSheet}
              key={item.name}
              onClick={() => setActiveSheet(index)}
              type="button"
            >
              {item.name}
            </button>
          ))}
        </div>
      ) : null}
      {mode === "semantic" && document.kind === "pptx" ? (
        <div className="flex items-center justify-center gap-2 border-b px-3 py-2">
          <Button
            aria-label="Previous slide"
            disabled={activeSlide === 0}
            onClick={() => setActiveSlide((value) => Math.max(0, value - 1))}
            size="icon-xs"
            type="button"
            variant="ghost"
          >
            <ChevronLeft />
          </Button>
          <span className="text-xs text-muted-foreground">
            Slide {activeSlide + 1} of {document.slides.length}
          </span>
          <Button
            aria-label="Next slide"
            disabled={activeSlide >= document.slides.length - 1}
            onClick={() =>
              setActiveSlide((value) =>
                Math.min(document.slides.length - 1, value + 1),
              )
            }
            size="icon-xs"
            type="button"
            variant="ghost"
          >
            <ChevronRight />
          </Button>
        </div>
      ) : null}
      <div className="min-h-0 flex-1 overflow-auto">
        <div className="h-full origin-top-left" style={{ zoom }}>
          {mode === "fidelity" && preview.fidelity ? (
            <iframe
              className="h-full w-full border-0 bg-white"
              data-testid="workspace-office-fidelity"
              sandbox=""
              srcDoc={preview.fidelity.html}
              title={`Rendered preview of ${resource.filename}`}
            />
          ) : document.kind === "docx" ? (
            <DocxView blocks={document.blocks} query={query} />
          ) : document.kind === "xlsx" && sheet ? (
            <WorksheetView query={query} sheet={sheet} />
          ) : document.kind === "pptx" && slide ? (
            <SlideView slide={slide} />
          ) : (
            <p className="p-8 text-center text-sm text-muted-foreground">
              This package contains no previewable content.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

export type OfficePreviewFormat = "docx" | "xlsx" | "pptx";

export type DocxBlock =
  | { kind: "paragraph"; text: string }
  | { kind: "heading"; level: number; text: string }
  | { kind: "list"; text: string }
  | { kind: "table"; rows: string[][] };

export type WorksheetCell = {
  reference: string;
  value: string;
  formula?: string;
};

export type WorksheetPreview = {
  name: string;
  rows: WorksheetCell[][];
  truncated: boolean;
};

export type SlidePreview = {
  number: number;
  title: string;
  body: string[];
  notes: string[];
};

export type OfficePreview = {
  version: 1;
  format: OfficePreviewFormat;
  document:
    | {
        kind: "docx";
        blocks: DocxBlock[];
        links: { text: string; target: string }[];
      }
    | {
        kind: "xlsx";
        sheets: WorksheetPreview[];
        charts: { title: string; chartType: string }[];
      }
    | { kind: "pptx"; slides: SlidePreview[] };
  truncated: boolean;
  warnings: string[];
  fidelity?: {
    renderer: string;
    html: string;
    truncated: boolean;
  };
};

export function searchableOfficeText(preview: OfficePreview): string {
  if (preview.document.kind === "docx") {
    return preview.document.blocks
      .flatMap((block) =>
        block.kind === "table" ? block.rows.flat() : [block.text],
      )
      .join("\n");
  }
  if (preview.document.kind === "xlsx") {
    return preview.document.sheets
      .flatMap((sheet) =>
        sheet.rows.flatMap((row) =>
          row.flatMap((cell) => [
            cell.reference,
            cell.value,
            cell.formula ?? "",
          ]),
        ),
      )
      .join("\n");
  }
  return preview.document.slides
    .flatMap((slide) => [slide.title, ...slide.body, ...slide.notes])
    .join("\n");
}

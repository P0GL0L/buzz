export type ArtifactPreviewKind =
  | "image"
  | "markdown"
  | "pdf"
  | "text"
  | "unsupported";

export type ArtifactWorkspaceResource = {
  kind: "artifact";
  url: string;
  filename: string;
  mime?: string;
  size?: number;
};

export type BrowserWorkspaceResource = {
  kind: "browser";
  url: string;
  title?: string;
};

export type WorkspaceResource =
  | ArtifactWorkspaceResource
  | BrowserWorkspaceResource;

const MARKDOWN_EXTENSIONS = [".md", ".markdown", ".mdown"];
const IMAGE_EXTENSIONS = [".avif", ".gif", ".jpeg", ".jpg", ".png", ".webp"];
const TEXT_EXTENSIONS = [
  ".csv",
  ".json",
  ".log",
  ".txt",
  ".xml",
  ".yaml",
  ".yml",
];

function hasExtension(filename: string, extensions: readonly string[]) {
  const normalized = filename.trim().toLowerCase();
  return extensions.some((extension) => normalized.endsWith(extension));
}

export function classifyArtifactPreview({
  filename,
  mime,
}: Pick<ArtifactWorkspaceResource, "filename" | "mime">): ArtifactPreviewKind {
  const normalizedMime = mime?.split(";")[0]?.trim().toLowerCase();

  if (normalizedMime === "application/pdf" || /\.pdf$/i.test(filename)) {
    return "pdf";
  }
  if (
    normalizedMime?.startsWith("image/") ||
    hasExtension(filename, IMAGE_EXTENSIONS)
  ) {
    return "image";
  }
  if (
    normalizedMime === "text/markdown" ||
    normalizedMime === "text/x-markdown" ||
    hasExtension(filename, MARKDOWN_EXTENSIONS)
  ) {
    return "markdown";
  }
  if (
    normalizedMime?.startsWith("text/") ||
    normalizedMime === "application/json" ||
    normalizedMime === "application/xml" ||
    normalizedMime === "application/yaml" ||
    normalizedMime === "application/x-yaml" ||
    hasExtension(filename, TEXT_EXTENSIONS)
  ) {
    return "text";
  }
  return "unsupported";
}

export function normalizeBrowserUrl(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const candidate = /^[a-z][a-z\d+.-]*:/i.test(trimmed)
    ? trimmed
    : `https://${trimmed}`;

  try {
    const parsed = new URL(candidate);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return null;
    }
    return parsed.toString();
  } catch {
    return null;
  }
}

export function formatWorkspaceFileSize(bytes: number | undefined): string {
  if (bytes == null || !Number.isFinite(bytes) || bytes < 0) return "";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes / 1024;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size < 10 ? size.toFixed(1) : Math.round(size)} ${units[unitIndex]}`;
}

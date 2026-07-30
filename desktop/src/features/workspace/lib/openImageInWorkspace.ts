import { requestOpenWorkspaceResource } from "../openWorkspaceResourceEvent";

function imageFilename(src: string, alt: string | undefined) {
  let filename = alt?.trim() || "image";
  try {
    const urlFilename = new URL(src).pathname.split("/").pop();
    if (urlFilename) filename = decodeURIComponent(urlFilename);
  } catch {
    // Preserve the accessible alt-text fallback for non-URL sources.
  }
  return filename;
}

export function openImageInWorkspace(
  src: string | undefined,
  alt: string | undefined,
) {
  if (!src) return;
  requestOpenWorkspaceResource({
    kind: "artifact",
    url: src,
    filename: imageFilename(src, alt),
    mime: "image/*",
  });
}

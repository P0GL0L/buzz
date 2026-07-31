export const MAX_DICTATION_MS = 120_000;

export type ComposerDictationPhase =
  | "idle"
  | "recording"
  | "transcribing"
  | "cancelled"
  | "error";

export function shouldHandleDictationShortcut(
  event: Pick<
    KeyboardEvent,
    "altKey" | "ctrlKey" | "key" | "metaKey" | "shiftKey"
  >,
  isMac = navigator.platform.toLowerCase().includes("mac"),
): boolean {
  const primary = isMac ? event.metaKey : event.ctrlKey;
  return (
    primary &&
    event.shiftKey &&
    !event.altKey &&
    event.key.toLowerCase() === "d"
  );
}

export function formatDictationElapsed(elapsedMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(elapsedMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export function transcriptInsertionText(transcript: string): string {
  const normalized = transcript.trim().replace(/\s+/g, " ");
  return normalized ? `${normalized} ` : "";
}

import { formatDictationElapsed } from "@/features/messages/lib/composerDictation";
import type { useMessageComposerDictation } from "@/features/messages/lib/useMessageComposerDictation";
import { cn } from "@/shared/lib/cn";

type ComposerDictation = ReturnType<typeof useMessageComposerDictation>;

export function ComposerDictationStatus({
  dictation,
}: {
  dictation: ComposerDictation;
}) {
  if (dictation.phase === "idle") return null;
  return (
    <div
      aria-live="polite"
      className={cn(
        "mb-2 flex items-center justify-between gap-3 rounded-lg px-3 py-2 text-xs",
        dictation.phase === "error"
          ? "bg-destructive/10 text-destructive"
          : "bg-muted text-muted-foreground",
      )}
      data-testid="composer-dictation-status"
    >
      <span>
        {dictation.phase === "recording"
          ? `Recording ${formatDictationElapsed(dictation.elapsedMs)} / 2:00`
          : dictation.phase === "transcribing"
            ? "Transcribing locally…"
            : dictation.phase === "cancelled"
              ? "Dictation cancelled."
              : (dictation.error ?? "Dictation could not continue.")}
      </span>
      {dictation.phase === "recording" ? (
        <button
          className="shrink-0 underline"
          onClick={() => void dictation.cancel()}
          type="button"
        >
          Cancel
        </button>
      ) : dictation.phase !== "transcribing" ? (
        <button
          className="shrink-0 underline"
          onClick={dictation.dismiss}
          type="button"
        >
          Dismiss
        </button>
      ) : null}
    </div>
  );
}

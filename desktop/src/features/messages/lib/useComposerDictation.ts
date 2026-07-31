import { invoke } from "@tauri-apps/api/core";
import * as React from "react";

import { createMicrophoneCapture } from "@/shared/lib/microphoneCapture";
import {
  MAX_DICTATION_MS,
  type ComposerDictationPhase,
} from "./composerDictation";

type DictationResult = {
  version: number;
  completionState: "completed" | "empty" | "timeout";
  transcript: string;
  durationMs: number;
  reason: string | null;
};

type ActiveCapture = {
  capture: Awaited<ReturnType<typeof createMicrophoneCapture>>;
  stream: MediaStream;
};

function errorMessage(cause: unknown): string {
  if (cause instanceof DOMException && cause.name === "NotAllowedError") {
    return "Microphone permission was denied. You can keep typing normally.";
  }
  if (cause instanceof DOMException && cause.name === "NotFoundError") {
    return "No microphone is available.";
  }
  return cause instanceof Error ? cause.message : String(cause);
}

export function useComposerDictation({
  disabled,
  onTranscript,
}: {
  disabled: boolean;
  onTranscript: (transcript: string) => void;
}) {
  const [phase, setPhase] = React.useState<ComposerDictationPhase>("idle");
  const [elapsedMs, setElapsedMs] = React.useState(0);
  const [error, setError] = React.useState<string | null>(null);
  const activeRef = React.useRef<ActiveCapture | null>(null);
  const busyRef = React.useRef(false);
  const startedAtRef = React.useRef(0);
  const mountedRef = React.useRef(true);
  const onTranscriptRef = React.useRef(onTranscript);
  onTranscriptRef.current = onTranscript;

  const stopMedia = React.useCallback(() => {
    const active = activeRef.current;
    activeRef.current = null;
    if (!active) return;
    active.stream.getTracks().forEach((track) => {
      track.onended = null;
      track.stop();
    });
    active.capture.stop();
  }, []);

  const cancel = React.useCallback(async () => {
    stopMedia();
    busyRef.current = false;
    try {
      await invoke("cancel_composer_dictation");
    } catch {
      // Local media cleanup is authoritative even if the backend is exiting.
    }
    if (mountedRef.current) {
      setPhase("cancelled");
      setElapsedMs(0);
    }
  }, [stopMedia]);

  const finish = React.useCallback(async () => {
    if (phase !== "recording" || busyRef.current) return;
    busyRef.current = true;
    stopMedia();
    setPhase("transcribing");
    try {
      const result = await invoke<DictationResult>("finish_composer_dictation");
      if (!mountedRef.current) return;
      if (result.completionState === "completed" && result.transcript.trim()) {
        onTranscriptRef.current(result.transcript);
        setPhase("idle");
        setError(null);
        setElapsedMs(0);
      } else {
        setPhase("error");
        setError(result.reason ?? "No speech was detected.");
      }
    } catch (cause) {
      if (!mountedRef.current) return;
      setPhase("error");
      setError(errorMessage(cause));
    } finally {
      busyRef.current = false;
    }
  }, [phase, stopMedia]);

  const start = React.useCallback(async () => {
    if (
      disabled ||
      busyRef.current ||
      phase === "recording" ||
      phase === "transcribing"
    ) {
      return;
    }
    busyRef.current = true;
    setError(null);
    setElapsedMs(0);
    try {
      await invoke("start_composer_dictation");
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: true,
          noiseSuppression: true,
          sampleRate: 48_000,
        },
      });
      const track = stream.getAudioTracks()[0];
      if (!track) {
        stream.getTracks().forEach((candidate) => {
          candidate.stop();
        });
        throw new Error("No microphone audio track is available.");
      }
      const capture = await createMicrophoneCapture(track, {
        command: "push_dictation_pcm",
      });
      activeRef.current = { capture, stream };
      track.onended = () => {
        if (!activeRef.current) return;
        stopMedia();
        void invoke("cancel_composer_dictation").catch(() => {});
        if (mountedRef.current) {
          setPhase("error");
          setError("The microphone disconnected while recording.");
        }
      };
      startedAtRef.current = performance.now();
      if (mountedRef.current) setPhase("recording");
    } catch (cause) {
      stopMedia();
      void invoke("cancel_composer_dictation").catch(() => {});
      if (mountedRef.current) {
        setPhase("error");
        setError(errorMessage(cause));
      }
    } finally {
      busyRef.current = false;
    }
  }, [disabled, phase, stopMedia]);

  const toggle = React.useCallback(() => {
    if (phase === "recording") {
      void finish();
    } else {
      void start();
    }
  }, [finish, phase, start]);

  const dismiss = React.useCallback(() => {
    if (phase === "recording" || phase === "transcribing") return;
    setPhase("idle");
    setError(null);
    setElapsedMs(0);
    void invoke("reset_composer_dictation_status").catch(() => {});
  }, [phase]);

  React.useEffect(() => {
    if (phase !== "recording") return;
    const timer = window.setInterval(() => {
      const elapsed = performance.now() - startedAtRef.current;
      setElapsedMs(elapsed);
      if (elapsed >= MAX_DICTATION_MS) {
        window.clearInterval(timer);
        void finish();
      }
    }, 250);
    return () => window.clearInterval(timer);
  }, [finish, phase]);

  React.useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      stopMedia();
      void invoke("cancel_composer_dictation").catch(() => {});
    };
  }, [stopMedia]);

  return {
    cancel,
    dismiss,
    elapsedMs,
    error,
    phase,
    toggle,
  };
}

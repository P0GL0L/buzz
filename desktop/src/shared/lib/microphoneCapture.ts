/**
 * Shared bounded microphone-to-Tauri PCM transport.
 *
 * Huddles and composer dictation choose separate Rust commands and lifecycle
 * owners, but use the same AudioWorklet capture mechanics. The browser never
 * retains raw audio.
 */

export type MicrophoneCaptureHandle = {
  stop: () => void;
  setGain: (value: number) => void;
  setTransmitting: (active: boolean) => void;
};

function invokeRawBinary(
  command: string,
  payload: Uint8Array,
): Promise<unknown> {
  // biome-ignore lint/suspicious/noExplicitAny: Tauri raw IPC is intentionally isolated here
  const internals = (window as any).__TAURI_INTERNALS__;
  if (!internals?.invoke) {
    return Promise.reject(
      new Error("Tauri microphone transport is unavailable"),
    );
  }
  return internals.invoke(command, payload);
}

export async function createMicrophoneCapture(
  audioTrack: MediaStreamTrack,
  {
    command,
    initialTransmitting = true,
  }: {
    command: "push_audio_pcm" | "push_dictation_pcm";
    initialTransmitting?: boolean;
  },
): Promise<MicrophoneCaptureHandle> {
  const audioContext = new AudioContext({ sampleRate: 48_000 });
  try {
    if (audioContext.state === "suspended") {
      await audioContext.resume();
    }
    await audioContext.audioWorklet.addModule("/worklet.js");

    const source = audioContext.createMediaStreamSource(
      new MediaStream([audioTrack]),
    );
    const gainNode = audioContext.createGain();
    const workletNode = new AudioWorkletNode(audioContext, "stt-tap-processor");
    source.connect(gainNode);
    gainNode.connect(workletNode);

    if (!initialTransmitting) {
      workletNode.port.postMessage({ type: "ptt", active: false });
    }

    let stopped = false;
    workletNode.port.onmessage = (event: MessageEvent<Float32Array>) => {
      if (stopped) return;
      const float32 = event.data;
      void invokeRawBinary(
        command,
        new Uint8Array(float32.buffer, float32.byteOffset, float32.byteLength),
      ).catch(() => {
        // Rust applies the bounded queue and reports lifecycle failures through
        // the controlling command. Dropping a late PCM frame is intentional.
      });
    };

    return {
      stop: () => {
        if (stopped) return;
        stopped = true;
        workletNode.port.onmessage = null;
        source.disconnect();
        gainNode.disconnect();
        workletNode.disconnect();
        void audioContext.close();
      },
      setGain: (value) => {
        gainNode.gain.value = Math.max(0, Math.min(1, value));
      },
      setTransmitting: (active) => {
        workletNode.port.postMessage({ type: "ptt", active });
      },
    };
  } catch (error) {
    void audioContext.close();
    throw error;
  }
}

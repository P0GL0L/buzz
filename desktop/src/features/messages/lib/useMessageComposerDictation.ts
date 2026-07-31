import type { Editor } from "@tiptap/react";
import * as React from "react";

import {
  shouldHandleDictationShortcut,
  transcriptInsertionText,
} from "./composerDictation";
import { useComposerDictation } from "./useComposerDictation";

export function useMessageComposerDictation({
  disabled,
  editor,
  scrollToBottom,
}: {
  disabled: boolean;
  editor: Editor | null;
  scrollToBottom: () => void;
}) {
  const onTranscript = React.useCallback(
    (transcript: string) => {
      const insertion = transcriptInsertionText(transcript);
      if (!insertion || !editor) return;
      editor.chain().focus().insertContent(insertion).run();
      scrollToBottom();
    },
    [editor, scrollToBottom],
  );
  const dictation = useComposerDictation({ disabled, onTranscript });
  const onKeyDownCapture = React.useCallback<
    React.KeyboardEventHandler<HTMLElement>
  >(
    (event) => {
      if (!shouldHandleDictationShortcut(event.nativeEvent) || disabled) return;
      event.preventDefault();
      event.stopPropagation();
      dictation.toggle();
    },
    [dictation.toggle, disabled],
  );
  return { ...dictation, onKeyDownCapture };
}

import * as React from "react";

import {
  AUXILIARY_PANEL_MIN_WIDTH_PX,
  clampAuxiliaryPanelWidth,
} from "@/shared/layout/auxiliaryPanelLayout";

const WORKSPACE_PANEL_WIDTH_SESSION_KEY =
  "asv-buzz.desktop.workspace-panel-width";
const WORKSPACE_PANEL_DEFAULT_WIDTH_PX = 640;

function getViewportWidth(): number {
  return typeof window === "undefined" ? 0 : window.innerWidth;
}

function clampWorkspacePanelWidth(width: number): number {
  return clampAuxiliaryPanelWidth(width, getViewportWidth());
}

function getInitialWorkspacePanelWidth(): number {
  if (typeof window === "undefined") return WORKSPACE_PANEL_DEFAULT_WIDTH_PX;

  try {
    const raw = window.sessionStorage.getItem(
      WORKSPACE_PANEL_WIDTH_SESSION_KEY,
    );
    if (!raw) return WORKSPACE_PANEL_DEFAULT_WIDTH_PX;

    const parsed = Number.parseInt(raw, 10);
    return Number.isFinite(parsed)
      ? clampWorkspacePanelWidth(parsed)
      : WORKSPACE_PANEL_DEFAULT_WIDTH_PX;
  } catch {
    return WORKSPACE_PANEL_DEFAULT_WIDTH_PX;
  }
}

export function useWorkspacePanelWidth() {
  const [widthPx, setWidthPx] = React.useState(getInitialWorkspacePanelWidth);
  const finishResizeRef = React.useRef<(() => void) | null>(null);

  React.useEffect(() => {
    try {
      window.sessionStorage.setItem(
        WORKSPACE_PANEL_WIDTH_SESSION_KEY,
        String(widthPx),
      );
    } catch {
      // Keep the in-memory width when session storage is unavailable.
    }
  }, [widthPx]);

  React.useEffect(
    () => () => {
      finishResizeRef.current?.();
    },
    [],
  );

  const onResizeStart = React.useCallback(
    (event: React.PointerEvent<HTMLButtonElement>) => {
      event.preventDefault();
      event.stopPropagation();

      finishResizeRef.current?.();

      const startX = event.clientX;
      const startWidth = widthPx;
      const pointerId = event.pointerId;
      const target = event.currentTarget;
      const previousCursor = document.body.style.cursor;
      const previousUserSelect = document.body.style.userSelect;
      let finished = false;

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      document.documentElement.dataset.workspaceResizing = "true";
      try {
        target.setPointerCapture(pointerId);
      } catch {
        // Window-level listeners below remain the fallback for older webviews.
      }

      const handlePointerMove = (moveEvent: PointerEvent) => {
        if (moveEvent.pointerId !== pointerId) return;
        const deltaX = startX - moveEvent.clientX;
        setWidthPx(clampWorkspacePanelWidth(startWidth + deltaX));
      };
      const finishResize = () => {
        if (finished) return;
        finished = true;
        document.body.style.cursor = previousCursor;
        document.body.style.userSelect = previousUserSelect;
        delete document.documentElement.dataset.workspaceResizing;
        window.removeEventListener("pointermove", handlePointerMove);
        window.removeEventListener("pointerup", handlePointerUp);
        window.removeEventListener("pointercancel", handlePointerCancel);
        window.removeEventListener("blur", finishResize);
        window.removeEventListener("keydown", handleKeyDown, true);
        target.removeEventListener("lostpointercapture", finishResize);
        try {
          if (target.hasPointerCapture(pointerId)) {
            target.releasePointerCapture(pointerId);
          }
        } catch {
          // Pointer capture may already have been released by the platform.
        }
        if (finishResizeRef.current === finishResize) {
          finishResizeRef.current = null;
        }
      };
      const handlePointerUp = (upEvent: PointerEvent) => {
        if (upEvent.pointerId === pointerId) finishResize();
      };
      const handlePointerCancel = (cancelEvent: PointerEvent) => {
        if (cancelEvent.pointerId === pointerId) finishResize();
      };
      const handleKeyDown = (keyEvent: KeyboardEvent) => {
        if (keyEvent.key !== "Escape") return;
        keyEvent.preventDefault();
        setWidthPx(startWidth);
        finishResize();
      };

      finishResizeRef.current = finishResize;
      window.addEventListener("pointermove", handlePointerMove);
      window.addEventListener("pointerup", handlePointerUp);
      window.addEventListener("pointercancel", handlePointerCancel);
      window.addEventListener("blur", finishResize, { once: true });
      window.addEventListener("keydown", handleKeyDown, true);
      target.addEventListener("lostpointercapture", finishResize, {
        once: true,
      });
    },
    [widthPx],
  );

  const onResetWidth = React.useCallback(() => {
    setWidthPx(WORKSPACE_PANEL_DEFAULT_WIDTH_PX);
  }, []);

  return {
    canReset: widthPx !== WORKSPACE_PANEL_DEFAULT_WIDTH_PX,
    maxWidth: `calc(100% - ${AUXILIARY_PANEL_MIN_WIDTH_PX}px)`,
    onResetWidth,
    onResizeStart,
    widthPx,
  };
}

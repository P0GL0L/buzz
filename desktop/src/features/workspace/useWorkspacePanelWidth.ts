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

  const onResizeStart = React.useCallback(
    (event: React.PointerEvent<HTMLButtonElement>) => {
      event.preventDefault();

      const startX = event.clientX;
      const startWidth = widthPx;
      const previousCursor = document.body.style.cursor;
      const previousUserSelect = document.body.style.userSelect;

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";

      const handlePointerMove = (moveEvent: PointerEvent) => {
        const deltaX = startX - moveEvent.clientX;
        setWidthPx(clampWorkspacePanelWidth(startWidth + deltaX));
      };
      const handlePointerUp = () => {
        document.body.style.cursor = previousCursor;
        document.body.style.userSelect = previousUserSelect;
        window.removeEventListener("pointermove", handlePointerMove);
      };

      window.addEventListener("pointermove", handlePointerMove);
      window.addEventListener("pointerup", handlePointerUp, { once: true });
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

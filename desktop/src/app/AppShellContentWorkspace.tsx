import type * as React from "react";
import { Outlet } from "@tanstack/react-router";

import * as BuzzTheme from "@/app/BuzzThemeSurfaces";
import { AppShellWorkspace } from "@/features/workspace/ui/AppShellWorkspace";
import { MainInsetProvider } from "@/shared/layout/MainInsetContext";
import { chromeCssVarDefaults } from "@/shared/layout/chromeLayout";
import { SidebarInset } from "@/shared/ui/sidebar";

export function AppShellContentWorkspace({
  mainInsetRef,
}: {
  mainInsetRef: React.RefObject<HTMLElement | null>;
}) {
  return (
    <div className="flex min-h-0 min-w-0 flex-1 overflow-hidden">
      <MainInsetProvider mainInsetRef={mainInsetRef}>
        <SidebarInset
          ref={mainInsetRef}
          className="isolate min-h-0 min-w-0 overflow-hidden bg-sidebar"
          data-buzz-glass-inset
          data-buzz-shadow-viewport
          data-testid="main-content-pane"
          style={chromeCssVarDefaults as React.CSSProperties}
        >
          <BuzzTheme.ContentSurface>
            <Outlet />
          </BuzzTheme.ContentSurface>
        </SidebarInset>
      </MainInsetProvider>
      <AppShellWorkspace />
    </div>
  );
}

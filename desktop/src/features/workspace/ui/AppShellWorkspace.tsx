import * as React from "react";

import {
  consumePendingWorkspaceResource,
  subscribeWorkspaceResource,
} from "../openWorkspaceResourceEvent";
import type { WorkspaceResource } from "../lib/workspaceResource";

const WorkspacePanel = React.lazy(async () => {
  const module = await import("./WorkspacePanel");
  return { default: module.WorkspacePanel };
});

/** Keeps the selected workspace resource docked beside the active app context. */
export function AppShellWorkspace() {
  const [resource, setResource] = React.useState<WorkspaceResource | null>(
    null,
  );

  React.useEffect(() => {
    const pending = consumePendingWorkspaceResource();
    if (pending) setResource(pending);
    return subscribeWorkspaceResource(setResource);
  }, []);

  if (!resource) return null;

  return (
    <React.Suspense fallback={null}>
      <WorkspacePanel onClose={() => setResource(null)} resource={resource} />
    </React.Suspense>
  );
}

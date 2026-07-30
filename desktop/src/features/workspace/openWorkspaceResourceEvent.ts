import type { WorkspaceResource } from "./lib/workspaceResource";

const OPEN_WORKSPACE_RESOURCE_EVENT = "buzz:open-workspace-resource";

let pendingResource: WorkspaceResource | null = null;

export function requestOpenWorkspaceResource(resource: WorkspaceResource) {
  pendingResource = resource;
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(OPEN_WORKSPACE_RESOURCE_EVENT));
  }
}

export function consumePendingWorkspaceResource(): WorkspaceResource | null {
  const resource = pendingResource;
  pendingResource = null;
  return resource;
}

export function subscribeWorkspaceResource(
  handler: (resource: WorkspaceResource) => void,
): () => void {
  function handleEvent() {
    const resource = consumePendingWorkspaceResource();
    if (resource) handler(resource);
  }

  window.addEventListener(OPEN_WORKSPACE_RESOURCE_EVENT, handleEvent);
  return () =>
    window.removeEventListener(OPEN_WORKSPACE_RESOURCE_EVENT, handleEvent);
}

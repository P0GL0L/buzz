import * as React from "react";

import type { Channel } from "@/shared/api/types";
import type { CreateChannelInput } from "@/features/sidebar/lib/useCreateChannelForm";
import type { WorkspaceResource } from "@/features/workspace/lib/workspaceResource";
import {
  consumePendingWorkspaceResource,
  subscribeWorkspaceResource,
} from "@/features/workspace/openWorkspaceResourceEvent";
import { useDeferredModalOpen } from "@/shared/ui/deferredModalOpen";

const ChannelBrowserDialog = React.lazy(async () => {
  const module = await import("@/features/channels/ui/ChannelBrowserDialog");
  return { default: module.ChannelBrowserDialog };
});

const ChannelManagementSheet = React.lazy(async () => {
  const module = await import("@/features/channels/ui/ChannelManagementSheet");
  return { default: module.ChannelManagementSheet };
});

const WorkspacePanel = React.lazy(async () => {
  const module = await import("@/features/workspace/ui/WorkspacePanel");
  return { default: module.WorkspacePanel };
});

export type BrowseDialogType = "stream" | "forum" | null;

type AppShellOverlaysProps = {
  activeChannel: Channel | null;
  browseDialogType: BrowseDialogType;
  channels: Channel[];
  currentPubkey?: string;
  isChannelManagementOpen: boolean;
  isCreatingBrowseChannel?: boolean;
  onBrowseChannelJoin: (channelId: string) => Promise<void>;
  onBrowseChannelCreate?: (input: CreateChannelInput) => Promise<void>;
  onBrowseDialogOpenChange: (open: boolean) => void;
  onChannelManagementOpenChange: (open: boolean) => void;
  onDeleteActiveChannel: () => void;
  onSelectChannel: (channelId: string) => void;
};

export function AppShellOverlays({
  activeChannel,
  browseDialogType,
  channels,
  currentPubkey,
  isChannelManagementOpen,
  isCreatingBrowseChannel,
  onBrowseChannelJoin,
  onBrowseChannelCreate,
  onBrowseDialogOpenChange,
  onChannelManagementOpenChange,
  onDeleteActiveChannel,
  onSelectChannel,
}: AppShellOverlaysProps) {
  const [visibleBrowseDialogType, setVisibleBrowseDialogType] =
    React.useState<BrowseDialogType>(null);
  const [workspaceResource, setWorkspaceResource] =
    React.useState<WorkspaceResource | null>(null);
  const { cancelDeferredModalOpen, openNextFrame: openModalNextFrame } =
    useDeferredModalOpen();

  React.useEffect(() => {
    if (browseDialogType === null) {
      cancelDeferredModalOpen();
      setVisibleBrowseDialogType(null);
      return;
    }

    setVisibleBrowseDialogType(null);
    openModalNextFrame(() => {
      setVisibleBrowseDialogType(browseDialogType);
    });
  }, [browseDialogType, cancelDeferredModalOpen, openModalNextFrame]);

  React.useEffect(() => {
    const pending = consumePendingWorkspaceResource();
    if (pending) setWorkspaceResource(pending);
    return subscribeWorkspaceResource(setWorkspaceResource);
  }, []);

  const renderedBrowseDialogType = visibleBrowseDialogType ?? browseDialogType;

  return (
    <>
      {browseDialogType !== null ? (
        <React.Suspense fallback={null}>
          <ChannelBrowserDialog
            channels={channels}
            channelTypeFilter={renderedBrowseDialogType ?? browseDialogType}
            isCreatingChannel={isCreatingBrowseChannel}
            onCreateChannel={onBrowseChannelCreate}
            onJoinChannel={onBrowseChannelJoin}
            onOpenChange={onBrowseDialogOpenChange}
            onSelectChannel={onSelectChannel}
            open={visibleBrowseDialogType !== null}
          />
        </React.Suspense>
      ) : null}

      {isChannelManagementOpen && activeChannel !== null ? (
        <React.Suspense fallback={null}>
          <ChannelManagementSheet
            channel={activeChannel}
            currentPubkey={currentPubkey}
            onDeleted={onDeleteActiveChannel}
            onOpenChange={onChannelManagementOpenChange}
            open={true}
          />
        </React.Suspense>
      ) : null}

      {workspaceResource ? (
        <React.Suspense fallback={null}>
          <WorkspacePanel
            onClose={() => setWorkspaceResource(null)}
            resource={workspaceResource}
          />
        </React.Suspense>
      ) : null}
    </>
  );
}

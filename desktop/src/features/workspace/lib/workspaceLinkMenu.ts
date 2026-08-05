import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "sonner";
import { copyTextToClipboard } from "@/shared/lib/clipboard";
import { requestOpenWorkspaceResource } from "../openWorkspaceResourceEvent";

type WorkspaceLinkMenuOptions = {
  href: string;
  label: string;
  onClose: () => void;
};

export function workspaceLinkMenuItems({
  href,
  label,
  onClose,
}: WorkspaceLinkMenuOptions) {
  return [
    {
      label: "Open in Buzz browser",
      onSelect: () => {
        onClose();
        requestOpenWorkspaceResource({
          kind: "browser",
          url: href,
          title: label || undefined,
        });
      },
    },
    {
      label: "Open in system browser",
      onSelect: () => {
        onClose();
        void openUrl(href).catch(() => {
          toast.error("Failed to open link");
        });
      },
    },
    {
      label: "Copy link",
      onSelect: () => {
        onClose();
        copyTextToClipboard(href, "Link copied to clipboard");
      },
    },
  ];
}

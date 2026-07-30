export type BrowserBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type BrowserAction = {
  version: 1;
  id: string;
  occurredAt: string;
  actor: string;
  action: string;
  target?: string;
  outcome: string;
};

export type NativeWorkspaceBrowserState = {
  version: 1;
  mode: "native";
  currentUrl?: string;
  title?: string;
  loading: boolean;
  activeAgent?: string;
  history: string[];
  historyIndex: number;
  actions: BrowserAction[];
  error?: string;
};

export type BrowserPageExtraction = {
  version: 1;
  url: string;
  title: string;
  text: string;
  links: { text: string; url: string }[];
  truncated: boolean;
};

export function boundsForElement(element: HTMLElement): BrowserBounds {
  const rect = element.getBoundingClientRect();
  return {
    x: Math.round(rect.left),
    y: Math.round(rect.top),
    width: Math.max(160, Math.round(rect.width)),
    height: Math.max(120, Math.round(rect.height)),
  };
}

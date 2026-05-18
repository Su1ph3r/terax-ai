/**
 * Action registry types — see docs/phase-2-palette-design.md.
 *
 * An Action is the canonical executable thing in Terax. The command
 * palette (Cmd+Shift+P) dispatches actions; future shortcuts and quick
 * actions will share this surface so we have a single dispatch path.
 */

export type ActionId = string;

/**
 * Categories group actions in the palette when the search input is
 * empty. Order roughly matches the order they appear.
 */
export type ActionCategory =
  | "general"
  | "tabs"
  | "panes"
  | "editor"
  | "terminal"
  | "ai"
  | "view"
  | "workspace"
  | "workflow"
  | "quick";

export const CATEGORY_LABEL: Record<ActionCategory, string> = {
  general: "General",
  tabs: "Tabs",
  panes: "Panes",
  editor: "Editor",
  terminal: "Terminal",
  ai: "AI",
  view: "View",
  workspace: "Workspace",
  workflow: "Workflows",
  quick: "Quick Actions",
};

export const CATEGORY_ORDER: ActionCategory[] = [
  "general",
  "tabs",
  "panes",
  "editor",
  "terminal",
  "ai",
  "view",
  "workspace",
  "workflow",
  "quick",
];

export type Action = {
  id: ActionId;
  label: string;
  category: ActionCategory;
  /** Subtitle shown under the label. */
  detail?: string;
  /** Extra terms folded into fuzzy search (separated by space). */
  keywords?: string;
  run: () => void | Promise<void>;
};

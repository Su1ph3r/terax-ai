import { useMemo } from "react";
import type { ShortcutHandlers, ShortcutId } from "@/modules/shortcuts";
import type { Action, ActionCategory } from "./types";

type CoreMeta = {
  id: ShortcutId;
  label: string;
  category: ActionCategory;
  detail?: string;
  keywords?: string;
};

const CORE_ACTION_META: ReadonlyArray<CoreMeta> = [
  // General
  {
    id: "settings.open",
    label: "Open Settings",
    category: "general",
    keywords: "preferences config",
  },
  {
    id: "shortcuts.open",
    label: "Show Keyboard Shortcuts",
    category: "general",
    keywords: "hotkeys keybindings",
  },

  // Tabs
  {
    id: "tab.new",
    label: "New Terminal Tab",
    category: "tabs",
    keywords: "shell open",
  },
  {
    id: "tab.newPrivate",
    label: "New Private Terminal",
    category: "tabs",
    keywords: "incognito ephemeral",
  },
  {
    id: "tab.newPreview",
    label: "New Preview Tab",
    category: "tabs",
    keywords: "browser web",
  },
  {
    id: "tab.newEditor",
    label: "New Editor Tab",
    category: "tabs",
    keywords: "file open code",
  },
  {
    id: "tab.close",
    label: "Close Tab",
    category: "tabs",
  },

  // Panes
  {
    id: "pane.splitRight",
    label: "Split Pane Right",
    category: "panes",
    keywords: "vertical horizontal",
  },
  {
    id: "pane.splitDown",
    label: "Split Pane Down",
    category: "panes",
    keywords: "vertical horizontal",
  },

  // View
  {
    id: "view.zoomIn",
    label: "Zoom In",
    category: "view",
  },
  {
    id: "view.zoomOut",
    label: "Zoom Out",
    category: "view",
  },
  {
    id: "view.zoomReset",
    label: "Reset Zoom",
    category: "view",
  },
  {
    id: "sidebar.toggle",
    label: "Toggle Sidebar",
    category: "view",
    keywords: "explorer",
  },

  // AI
  {
    id: "ai.toggle",
    label: "Toggle AI Panel",
    category: "ai",
    keywords: "chat assistant",
  },
];

/**
 * Compose the palette's action list from the various sources. Right now
 * that's just the core actions whose `run` callbacks point at the same
 * shortcut handlers. Phase 2.3 will also fold in workflows and Phase
 * 2.4 quick actions — keep adding sources here so the palette stays a
 * dumb consumer.
 */
export function useAppActions(handlers: ShortcutHandlers): Action[] {
  return useMemo<Action[]>(() => {
    const actions: Action[] = [];
    for (const meta of CORE_ACTION_META) {
      const handler = handlers[meta.id];
      if (!handler) continue;
      actions.push({
        ...meta,
        // Shortcut handlers receive a KeyboardEvent. The palette doesn't
        // have one — synthesize an inert event so handlers that look at
        // `e.preventDefault()` etc. don't crash.
        run: () => handler(new KeyboardEvent("keydown")),
      });
    }
    return actions;
  }, [handlers]);
}

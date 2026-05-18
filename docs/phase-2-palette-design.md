# Phase 2 — Command Palette, Workflows, Quick Actions

Design draft. **Not yet implemented.** Once approved, this folds into the implementation plan.

## 1. Vocabulary

| Term | Meaning |
|---|---|
| **Action** | A named operation registered in the action registry, executable from the palette and (optionally) bound to a shortcut. e.g. `"tab.new"`, `"workflow.run:rebuild-and-deploy"`. |
| **Action provider** | Module that registers actions with the registry. Built-in providers ship core app actions; user providers come from workflows. |
| **Palette** | The Cmd+Shift+P overlay that fuzzy-searches all registered actions. |
| **Workflow** | A user-defined parameterized terminal command. e.g. `"deploy [env]: ./bin/deploy --env={env}"`. |
| **Quick action** | A context-aware action surfaced separately. e.g. inside a terminal tab, suggest "Run last command in new tab", "Reveal cwd in explorer". |

Action and Workflow are distinct concepts:
- An **action** is the canonical executable thing (one in-memory entry in the registry).
- A **workflow** is *data* that gets compiled into actions at registration time. Every workflow produces one `workflow.run:<id>` action.

## 2. Action composition (implementation note)

> **Revision after Phase 2.1 shipped.** The original design called for a Zustand action registry that components register/unregister actions through. Two attempts at that pattern hit infinite-render loops driven by Zustand selectors returning fresh references on every read. We dropped the registry and replaced it with plain React data flow: a hook composes the action list (`useAppActions`) and the palette receives it as a prop. Phase 2.3 / 2.4 fold in workflows and quick actions by extending the hook, not by writing to a global store. The action / category types below still describe the shape; ignore the "registry" terminology in the rest of the doc.

```ts
// src/modules/palette/registry.ts
export type ActionId = string; // dotted, namespaced: "tab.new", "workflow.run:my-deploy"

export type ActionContext = {
  activeTabId: number | null;
  activeTabKind: "terminal" | "editor" | "preview" | null;
  activeWorkspaceEnv: WorkspaceEnv;
  selection: string | null;       // current text selection (terminal or editor)
  cwd: string | null;             // active terminal cwd, if any
};

export type Action = {
  id: ActionId;
  label: string;                  // "New Terminal Tab"
  detail?: string;                // "Open a new shell in the current workspace env"
  category: ActionCategory;       // for grouping/filtering
  keywords?: string[];            // for search ("tab open new shell")
  /** Optional: a stable icon name from the existing hugeicons set. */
  icon?: string;
  /** Whether the action makes sense right now. Defaults to always-visible. */
  isVisible?: (ctx: ActionContext) => boolean;
  /** Whether the action is enabled (greyed-out when false). Defaults true. */
  isEnabled?: (ctx: ActionContext) => boolean;
  run: (ctx: ActionContext) => void | Promise<void>;
};

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
```

**Singleton registry** (Zustand store): map of `ActionId → Action`. Components register on mount, unregister on unmount. Workflows re-register en masse on every workflow store change.

**Dispatch**: `dispatchAction(id, ctx)` looks up by id, runs it. Used by both the palette and the shortcuts system.

## 3. Palette UX

**Keybinding**: `Cmd/Ctrl+Shift+P` (registered via the existing shortcuts system, new id `palette.open`).

**Layout** (shadcn `Command` / cmdk-based, similar to the existing keyboard-shortcuts dialog):

```
┌─────────────────────────────────────────┐
│ > [search input]                        │
├─────────────────────────────────────────┤
│ Recent                                  │
│   New Terminal Tab            Ctrl+T    │
│   Open Settings               Ctrl+,    │
│                                         │
│ Tabs                                    │
│   ↻ New Terminal Tab          Ctrl+T    │
│   ▢ New Editor Tab            Ctrl+E    │
│                                         │
│ Workflows                               │
│   ⚡ deploy production                  │
│   ⚡ rebuild docker            (params)  │
│                                         │
│ Quick Actions                           │
│   ⤴ Reveal cwd in explorer              │
│   ↻ Run last command in new tab         │
└─────────────────────────────────────────┘
```

- **Search**: fuzzy over `id + label + detail + keywords`. Uses `fzf`-style scoring (cmdk has one built-in).
- **Recents**: last 5 dispatched action ids, stored in `localStorage`, shown above the categorized list when the search input is empty.
- **Categories**: shown as headers when scrolling/no-query. When the user types, results collapse into a single ranked list.
- **Keyboard**: ↑/↓ navigate, Enter run, Esc close.
- **Shortcut badges**: shown on the right when the action has a configured shortcut.

## 4. Workflow data model

```ts
// src/modules/workflows/types.ts
export type Workflow = {
  id: string;                     // wf-<base36>
  name: string;                   // "Deploy production"
  description?: string;
  command: string;                // "./bin/deploy --env=production"
  /**
   * Parameters get prompted as a small inline form after pick. Substituted
   * into `command` via `{paramName}` template placeholders.
   */
  parameters?: WorkflowParameter[];
  /**
   * "active-terminal"  → run in current terminal pane (default)
   * "new-tab"          → spawn a new terminal tab and run there
   * "background"       → shell_bg_spawn, surface output in a notification
   */
  target: "active-terminal" | "new-tab" | "background";
  /**
   * Optional cwd. Template placeholders allowed.
   *   - empty / unset → run wherever the target lands
   *   - "{cwd}"       → current terminal cwd
   *   - any literal path
   */
  cwd?: string;
};

export type WorkflowParameter = {
  name: string;                   // matches `{name}` placeholder
  label: string;                  // display label in the prompt
  placeholder?: string;           // input placeholder
  default?: string;
  /**
   * Optional dropdown options. When set, the parameter is a select
   * rather than a free-text input.
   */
  choices?: string[];
};
```

**Storage**: `terax-workflows.json` via `LazyStore`, mirrors the snippet pattern in `src/modules/ai/lib/snippets.ts`.

**Persistence**: `useWorkflowsStore` (Zustand) hydrates on mount, listens for cross-window changes via a Tauri event.

## 5. Quick actions

A **context provider** that exposes 3-5 ephemeral actions based on the current tab kind:

| Context | Actions |
|---|---|
| **Active terminal tab** | Reveal cwd in explorer · Run last command in new tab · Copy cwd · Toggle private mode (?) |
| **Active editor tab** | Reveal file in explorer · Open containing folder in terminal · Copy path · Copy relative path |
| **Active preview tab** | Open in external browser · Copy URL · Reload |

Generated on the fly from the `ActionContext` each time the palette opens. Implemented as a single hook `useQuickActions(ctx)` that returns a synthetic `Action[]` not stored in the registry.

## 6. Integration with the shortcuts system

Two integration points, kept minimal:

1. **Palette open shortcut** — add `palette.open` to `ShortcutId` + default binding `Cmd/Ctrl+Shift+P`.
2. **Action-bound shortcuts** — actions with stable ids that happen to match an existing `ShortcutId` (e.g. `tab.new`) can be invoked from BOTH the palette and the global shortcut. The palette imports the same handler the shortcuts hook does.

For new actions that should have a configurable shortcut, users go to **Settings → Shortcuts** (existing UI). Each action with `bindable: true` appears there. No separate keybinding UI for actions vs shortcuts.

## 7. UI surfaces touched

| File | Change |
|---|---|
| `src/modules/palette/` (new) | Registry + `CommandPalette.tsx` component + `useQuickActions.ts` |
| `src/modules/workflows/` (new) | Types, store, `WorkflowEditorDialog.tsx`, `lib/expand.ts` |
| `src/settings/sections/WorkflowsSection.tsx` (new) | Settings page tab — list, edit, delete workflows |
| `src/modules/shortcuts/shortcuts.ts` | Add `palette.open` + per-action ids that need keybindings |
| `src/app/App.tsx` | Mount `<CommandPalette />`; register core actions in a `useCoreActions()` hook |

## 8. Phasing

Suggested implementation order so we can ship incrementally:

| Phase | What | Mergeable on its own? |
|---|---|---|
| **2.1** | Action registry primitive + palette UI + 10 core actions (tab.new, settings.open, etc.) wired via the registry. Keybinding `Cmd+Shift+P`. | ✅ |
| **2.2** | Workflow types + store + Settings UI (list, edit, delete) + `WorkflowEditorDialog`. No palette wiring yet. | ✅ |
| **2.3** | Wire workflows into the palette: registered as `workflow.run:<id>` actions; parameter prompt; execution in target context. | ✅ (depends on 2.1 + 2.2) |
| **2.4** | Quick actions hook + integration into the palette as a third section. | ✅ (depends on 2.1) |
| **2.5** | Polish: recents in localStorage, fuzzy search tuning, icons, shortcuts badges, action-search keywords. | ✅ |

## 9. Settled decisions

1. **Default keybinding** — `Cmd/Ctrl+Shift+P` (VSCode-style). New `palette.open` ShortcutId; bindable like any other shortcut.
2. **Parameter prompting** — inline view inside the palette. After picking a workflow with parameters, cmdk transitions to a small form (label / input per param, Enter to run). Esc returns to the picker.
3. **Workflow `target` default** — `"active-terminal"`. The "saved command" mental model wins; users can switch per-workflow to `new-tab` or `background`.
4. **Workflow icons** — not in v1. Workflow rows use a generic ⚡ glyph. Add later if there's demand.
5. **Workflow scope** — **global AND per-workspace from day one**:
   - Global file: `terax-workflows.json` in the standard app config dir.
   - Per-workspace file: `.terax/workflows.json` inside the active workspace root, discovered on workspace switch.
   - Merge rule: both lists are concatenated and shown side-by-side in the palette; per-workspace entries are tagged `(workspace)` in the row's detail so users can tell the source.
   - Conflict (same `id`): per-workspace wins. (`id` is generated, collisions are vanishingly rare; conflict here is mostly a copy-paste-edit scenario.)
   - Settings UI: separate tabs in the Workflows section — "Global" / "This workspace". The latter is empty + disabled when there's no active workspace folder.
6. **Snippet integration** — workflows and AI snippets stay separate. Different storage (`terax-workflows.json` vs `terax-ai-snippets.json`), different stores, different palettes/composers. They have different lifecycles (snippets are AI prompt fragments; workflows are shell commands) and lumping them would muddy both.

Implementation begins at Phase 2.1.

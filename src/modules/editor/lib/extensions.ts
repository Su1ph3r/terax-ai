import { detectMonoFontFamily } from "@/lib/fonts";
import { indentUnit } from "@codemirror/language";
import { lintGutter } from "@codemirror/lint";
import { search } from "@codemirror/search";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";

// Compartments allow runtime reconfiguration without rebuilding state.
export const languageCompartment = new Compartment();
export const readOnlyCompartment = new Compartment();
export const wrapCompartment = new Compartment();
export const vimCompartment = new Compartment();

// Only what basicSetup doesn't already cover, to avoid duplicate extensions.
// basicSetup gives us line numbers, fold gutter, history, indentOnInput,
// bracketMatching, closeBrackets, autocompletion, highlightActiveLine,
// highlightSelectionMatches and the search keymap.
//
// Color choices (background, foreground, caret, selection, gutter, active
// line) intentionally live on the editor theme extension — see EDITOR_THEMES
// in modules/settings/store and the @uiw/codemirror-theme-* packages. Putting
// color overrides here would silently win over the user-picked theme because
// react-codemirror concatenates user extensions after the `theme` prop, and
// the last extension wins on CSS rules with the same selector (#32).
export function buildSharedExtensions(): Extension[] {
  return [
    indentUnit.of("  "),
    EditorState.tabSize.of(2),
    search({ top: true }),
    lintGutter(),
    EditorView.theme({
      "&, &.cm-editor, &.cm-editor.cm-focused": {
        outline: "none",
        padding: "8px",
      },
      ".cm-scroller": {
        fontFamily: detectMonoFontFamily(),
        fontSize: "13px",
        lineHeight: "1.55",
      },
      ".cm-gutter-lint": {
        width: "0px",
      },
      ".cm-foldGutter": { width: "10px" },
      ".cm-lineNumbers .cm-activeLineGutter": {
        userSelect: "none",
      },
      // Vim normal-mode block cursor — translucent foreground, no rose hue.
      // Keyed to app color tokens because Vim mode is an app-level feature,
      // not a per-theme one.
      ".cm-fat-cursor": {
        background:
          "color-mix(in srgb, var(--foreground) 35%, transparent) !important",
        outline:
          "1px solid color-mix(in srgb, var(--foreground) 55%, transparent) !important",
        color: "var(--foreground) !important",
      },
      "&:not(.cm-focused) .cm-fat-cursor": {
        background: "transparent !important",
        outline:
          "1px solid color-mix(in srgb, var(--foreground) 35%, transparent) !important",
      },
      // Search panel is an app UI surface — keep it matched to the app
      // chrome regardless of the editor theme.
      ".cm-panels": {
        backgroundColor: "var(--popover)",
        color: "var(--popover-foreground)",
        borderColor: "var(--border)",
      },
    }),
  ];
}

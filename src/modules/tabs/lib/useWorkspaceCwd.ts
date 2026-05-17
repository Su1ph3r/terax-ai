import { useCallback, useEffect, useMemo, useRef } from "react";
import type { WorkspaceEnv } from "@/modules/workspace";
import type { Tab } from "./useTabs";

type Result = {
  explorerRoot: string | null;
  inheritedCwdForNewTab: () => string | undefined;
};

/**
 * Check whether two workspace envs reference the same shell environment.
 * `local-local` matches; WSL only matches when the distro names line up.
 */
function envsMatch(a: WorkspaceEnv | undefined, b: WorkspaceEnv): boolean {
  if (!a) return b.kind === "local"; // Tab without an env defaults to Local.
  if (a.kind !== b.kind) return false;
  if (a.kind === "wsl" && b.kind === "wsl") return a.distro === b.distro;
  return true;
}

export function useWorkspaceCwd(
  activeTab: Tab | undefined,
  tabs: Tab[],
  home: string | null,
  /**
   * Current ambient workspace env (== file-tree / AI tools env). Used to
   * skip tab cwds that don't belong to this env — e.g. a Windows
   * `C:/Users/foo` cwd is invalid as a path under a WSL workspace and
   * produces ERROR_PATH_NOT_FOUND if we hand it to the backend's
   * `wsl_path_to_unc`.
   */
  workspaceEnv: WorkspaceEnv,
): Result {
  const lastTerminalCwd = useRef<string | null>(null);

  useEffect(() => {
    // Only cache the focused terminal's cwd when its env matches the
    // ambient one. Otherwise the cached value becomes a path that's
    // invalid under the now-current workspace.
    if (
      activeTab?.kind === "terminal" &&
      activeTab.cwd &&
      envsMatch(activeTab.workspace, workspaceEnv)
    ) {
      lastTerminalCwd.current = activeTab.cwd;
    }
  }, [activeTab, workspaceEnv]);

  const explorerRoot = useMemo<string | null>(() => {
    if (
      activeTab?.kind === "terminal" &&
      activeTab.cwd &&
      envsMatch(activeTab.workspace, workspaceEnv)
    )
      return activeTab.cwd;
    if (lastTerminalCwd.current) return lastTerminalCwd.current;
    const anyTerm = tabs.find(
      (t) =>
        t.kind === "terminal" &&
        t.cwd &&
        envsMatch(t.workspace, workspaceEnv),
    );
    if (anyTerm?.kind === "terminal" && anyTerm.cwd) return anyTerm.cwd;
    return home;
  }, [activeTab, tabs, home, workspaceEnv]);

  const inheritedCwdForNewTab = useCallback((): string | undefined => {
    if (
      activeTab?.kind === "terminal" &&
      activeTab.cwd &&
      envsMatch(activeTab.workspace, workspaceEnv)
    )
      return activeTab.cwd;
    // Editor tabs inherit the last terminal's cwd (or workspace home), not
    // the file's folder — opening a new terminal from a file shouldn't
    // hijack the user's working directory context.
    return lastTerminalCwd.current ?? home ?? undefined;
  }, [activeTab, home, workspaceEnv]);

  return { explorerRoot, inheritedCwdForNewTab };
}

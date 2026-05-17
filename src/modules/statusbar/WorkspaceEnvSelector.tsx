import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { IS_WINDOWS } from "@/lib/platform";
import {
  LOCAL_WORKSPACE,
  useWorkspaceEnvStore,
  type WorkspaceEnv,
} from "@/modules/workspace";
import { Refresh01Icon, ServerStack03Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";

type Props = {
  /**
   * Picked env. Becomes the seed for new `+` tabs AND the env that AI /
   * file explorer operate in (fetches the matching home so the file
   * tree doesn't error with ENOENT under a WSL workspace). Does NOT
   * close or re-spawn any open terminal tab.
   */
  onSelect: (env: WorkspaceEnv) => void;
  /**
   * Workspace of the currently focused terminal tab — shown as the
   * primary label so the user always sees what the focused terminal is
   * actually running in (distinct from the AI/explorer default).
   */
  activeTerminalWorkspace?: WorkspaceEnv;
};

export function WorkspaceEnvSelector({
  onSelect,
  activeTerminalWorkspace,
}: Props) {
  if (!IS_WINDOWS) return null;

  // Two distinct things to surface here:
  //   - what the focused terminal is running in (the visible label)
  //   - what new "+" tabs / AI / explorer will use (the "default" badge
  //     inside the dropdown)
  // Showing the focused tab as the label keeps the bar honest when the
  // user is staring at a WSL pwsh prompt — they shouldn't see "Windows"
  // up there.
  const defaultEnv = useWorkspaceEnvStore((s) => s.defaultEnv);
  const distros = useWorkspaceEnvStore((s) => s.distros);
  const loading = useWorkspaceEnvStore((s) => s.loading);
  const error = useWorkspaceEnvStore((s) => s.error);
  const refreshDistros = useWorkspaceEnvStore((s) => s.refreshDistros);

  const handleOpenChange = (open: boolean) => {
    if (open && distros.length === 0 && !loading) {
      void refreshDistros();
    }
  };

  const labelSource = activeTerminalWorkspace ?? defaultEnv;
  const label =
    labelSource.kind === "wsl" ? `WSL: ${labelSource.distro}` : "Windows";

  const isCurrentDefault = (candidate: WorkspaceEnv): boolean =>
    candidate.kind === defaultEnv.kind &&
    (candidate.kind === "local" ||
      (defaultEnv.kind === "wsl" && candidate.distro === defaultEnv.distro));

  return (
    <DropdownMenu onOpenChange={handleOpenChange}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex h-6 shrink-0 items-center gap-1 rounded-sm px-1.5 text-[11px] text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus:outline-none focus-visible:outline-none focus-visible:ring-0 data-[state=open]:bg-accent data-[state=open]:text-foreground"
          title={`Active terminal: ${label}. Pick to set the default for new tabs and AI / file explorer.`}
        >
          <HugeiconsIcon
            icon={ServerStack03Icon}
            size={13}
            strokeWidth={1.75}
          />
          <span className="max-w-28 truncate">{label}</span>
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-56">
        <div className="px-2 py-1 text-[10.5px] uppercase tracking-wider text-muted-foreground">
          Default for new tabs
        </div>
        <DropdownMenuItem onSelect={() => onSelect(LOCAL_WORKSPACE)}>
          <span className="flex-1">Windows Local</span>
          {isCurrentDefault(LOCAL_WORKSPACE) ? (
            <span className="text-[10px] text-muted-foreground">default</span>
          ) : null}
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {distros.length === 0 ? (
          <DropdownMenuItem disabled>
            {loading
              ? "Loading WSL distros..."
              : error
                ? "WSL unavailable"
                : "No WSL distros found"}
          </DropdownMenuItem>
        ) : (
          distros.map((distro) => {
            const candidate: WorkspaceEnv = {
              kind: "wsl",
              distro: distro.name,
            };
            return (
              <DropdownMenuItem
                key={distro.name}
                onSelect={() => onSelect(candidate)}
              >
                <span className="flex-1">WSL: {distro.name}</span>
                {isCurrentDefault(candidate) ? (
                  <span className="text-[10px] text-muted-foreground">
                    default
                  </span>
                ) : null}
              </DropdownMenuItem>
            );
          })
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={() => void refreshDistros()}>
          <HugeiconsIcon icon={Refresh01Icon} size={13} strokeWidth={1.75} />
          Refresh
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { useMemo, useState } from "react";
import {
  CATEGORY_LABEL,
  CATEGORY_ORDER,
  type Action,
  type ActionCategory,
} from "./types";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: Action[];
};

export function CommandPalette({ open, onOpenChange, actions }: Props) {
  const [query, setQuery] = useState("");

  // Group by category for the empty-query view. Once the user types,
  // cmdk's own scorer reorders results by score across all groups.
  const grouped = useMemo(() => groupByCategory(actions), [actions]);

  return (
    <CommandDialog
      open={open}
      onOpenChange={(next) => {
        if (!next) setQuery("");
        onOpenChange(next);
      }}
      title="Command Palette"
      description="Run an action by name."
    >
      <CommandInput
        placeholder="Type a command…"
        value={query}
        onValueChange={setQuery}
      />
      <CommandList>
        <CommandEmpty>No matching actions.</CommandEmpty>
        {grouped.map(({ category, items }) => (
          <CommandGroup key={category} heading={CATEGORY_LABEL[category]}>
            {items.map((a) => (
              <CommandItem
                key={a.id}
                value={`${a.label} ${a.detail ?? ""} ${a.keywords ?? ""} ${a.id}`}
                onSelect={() => {
                  onOpenChange(false);
                  setQuery("");
                  void a.run();
                }}
              >
                <span className="flex min-w-0 flex-col">
                  <span className="truncate">{a.label}</span>
                  {a.detail ? (
                    <span className="truncate text-[11px] text-muted-foreground">
                      {a.detail}
                    </span>
                  ) : null}
                </span>
              </CommandItem>
            ))}
          </CommandGroup>
        ))}
      </CommandList>
    </CommandDialog>
  );
}

function groupByCategory(
  actions: Action[],
): { category: ActionCategory; items: Action[] }[] {
  const buckets = new Map<ActionCategory, Action[]>();
  for (const a of actions) {
    const list = buckets.get(a.category);
    if (list) list.push(a);
    else buckets.set(a.category, [a]);
  }
  return CATEGORY_ORDER.filter((c) => buckets.has(c)).map((category) => ({
    category,
    items: buckets.get(category)!,
  }));
}

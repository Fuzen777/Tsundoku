import { useState } from "react"

/** The views with a grouped list (`components/ArrangedList.tsx`): each remembers the groups folded away in it. */
export type GroupedView = "pending" | "queue" | "watched"

/** The groups folded away in each view, by heading, kept while the app runs so leaving a view doesn't unfold them. */
const collapsedByView: Record<GroupedView, ReadonlySet<string>> = {
  pending: new Set(),
  queue: new Set(),
  watched: new Set(),
}

/** A view's folded groups: the headings' keys (`/<group key>`, a subgroup's after its group's). */
export interface FoldedGroups {
  collapsed: ReadonlySet<string>
  /** Folds or unfolds a heading, by its key. */
  toggle: (key: string) => void
  /** Whether a top-level group is folded, by its `ItemGroup.key`. */
  isFolded: (groupKey: string) => boolean
  /** Unfolds every group. */
  unfoldAll: () => void
}

/** The groups folded away in a view, remembered while the app runs. */
export function useFoldedGroups(view: GroupedView): FoldedGroups {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(collapsedByView[view])
  const update = (change: (current: ReadonlySet<string>) => ReadonlySet<string>) =>
    setCollapsed((current) => {
      const next = change(current)
      collapsedByView[view] = next
      return next
    })
  return {
    collapsed,
    toggle: (key) =>
      update((current) => {
        const next = new Set(current)
        if (!next.delete(key)) next.add(key)
        return next
      }),
    isFolded: (groupKey) => collapsed.has(`/${groupKey}`),
    unfoldAll: () => update(() => new Set()),
  }
}

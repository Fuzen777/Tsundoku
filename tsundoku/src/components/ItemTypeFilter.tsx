import type { TypeCount, TypeFilter } from "../listingPicker"
import { strings } from "../strings"
import { SegmentedControl } from "./SegmentedControl"

const text = strings.picker

interface ItemTypeFilterProps {
  /** The types among the items, with how many are of each. */
  counts: readonly TypeCount[]
  value: TypeFilter
  onChange: (type: TypeFilter) => void
  disabled?: boolean
}

/** Which type of items a picker shows: All, Images (32), Videos (18)… Offered only while its items are of several
 * types. */
export function ItemTypeFilter({ counts, value, onChange, disabled = false }: ItemTypeFilterProps) {
  if (counts.length < 2) return null
  return (
    <SegmentedControl<TypeFilter>
      label={text.type}
      options={[
        { value: "all", label: text.types.all, hint: text.typeHints.all },
        ...counts.map(({ type, count }) => ({
          value: type,
          label: text.typeCount(text.types[type], count),
          hint: text.typeHints[type],
        })),
      ]}
      value={value}
      onChange={onChange}
      disabled={disabled}
    />
  )
}

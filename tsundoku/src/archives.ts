import type { RuleUnpacking } from "./bindings/RuleUnpacking"
import type { UnpackPlace } from "./bindings/UnpackPlace"
import { strings } from "./strings"

const text = strings.settings

/** Where what an archive holds can go, in the order Settings → Archives' and a rule's toggles offer them. */
export const UNPACK_PLACES: readonly UnpackPlace[] = ["smart", "inPlace", "archiveFolder"]

/** A rule's or a watched source's Unpack Archives: its own choice, or Default, which leaves it to what decides after
 * it (the rules before a rule, then Settings → Archives). */
export type UnpackingChoice = RuleUnpacking | "default"

/** Default, Skip (left packed), then the places Settings → Archives offers. */
export const UNPACKING_CHOICES: readonly { value: UnpackingChoice; label: string }[] = [
  { value: "default", label: text.unpackChoices.default },
  { value: "off", label: text.unpackChoices.off },
  ...UNPACK_PLACES.map((place) => ({ value: place, label: text.unpackPlaceChoices[place] })),
]

/** A rule's or a watched source's own choice as a phrase: "Leaves archives packed", "Unpacks archives: In Place". */
export function describeUnpacking(unpacking: RuleUnpacking): string {
  return unpacking === "off" ? text.leavesArchivesPacked : text.unpacksArchives(text.unpackPlaces[unpacking])
}

import type { UnpackPlace } from "./bindings/UnpackPlace"

/** Where what an archive holds can go, in the order Settings → Archives' and a rule's toggles offer them. */
export const UNPACK_PLACES: readonly UnpackPlace[] = ["smart", "inPlace", "archiveFolder"]

import { strings } from "./strings"

/** A size in bytes as Windows shows it: in 1024-based units, with one decimal from KB up ("22.5 MB"). */
export function formatBytes(bytes: number): string {
  const units = strings.units.bytes
  let value = bytes
  let unitIndex = 0
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }
  return unitIndex === 0 ? `${value} ${units[0]}` : `${value.toFixed(1)} ${units[unitIndex]}`
}

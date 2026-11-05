import { useSyncExternalStore } from "react"

/**
 * Values that stream from the core, such as progress, kept outside React Query since they change several times a
 * second. Components read one value with `useValue`, and re-render only when it changes.
 */
export function createLiveStore<Key, Value>() {
  const values = new Map<Key, Value>()
  const listeners = new Set<() => void>()
  const notify = () => {
    for (const listener of listeners) listener()
  }
  const subscribe = (listener: () => void) => {
    listeners.add(listener)
    return () => {
      listeners.delete(listener)
    }
  }
  return {
    set(key: Key, value: Value) {
      values.set(key, value)
      notify()
    },
    delete(key: Key) {
      if (values.delete(key)) notify()
    },
    useValue(key: Key): Value | undefined {
      return useSyncExternalStore(subscribe, () => values.get(key))
    },
  }
}

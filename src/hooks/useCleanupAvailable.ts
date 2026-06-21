import { useEffect, useState } from "react";
import { commands } from "@/bindings";

/**
 * Whether on-device transcription cleanup (Apple FoundationModels) is available
 * on this machine, backed by the `is_cleanup_available` backend command.
 *
 * Availability is queried once and cached at module level so every consumer
 * shares a single backend round-trip. The hook returns `null` until the first
 * query settles, then `true` / `false`. On failure it resolves to `false` so
 * the cleanup toggle stays hidden rather than dangling in a loading state.
 */

let cache: boolean | undefined;
let inflight: Promise<boolean> | undefined;

function fetchAvailability(): Promise<boolean> {
  if (cache !== undefined) return Promise.resolve(cache);
  if (!inflight) {
    inflight = commands
      .isCleanupAvailable()
      .then((available) => {
        cache = available;
        return available;
      })
      .catch(() => {
        cache = false;
        return false;
      });
  }
  return inflight;
}

export function useCleanupAvailable(): boolean | null {
  const [available, setAvailable] = useState<boolean | null>(cache ?? null);

  useEffect(() => {
    if (available !== null) return;
    let cancelled = false;
    void fetchAvailability().then((result) => {
      if (!cancelled) setAvailable(result);
    });
    return () => {
      cancelled = true;
    };
  }, [available]);

  return available;
}

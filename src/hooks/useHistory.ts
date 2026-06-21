import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type HistoryEntry } from "@/bindings";

const POLL_INTERVAL_MS = 3000;

export interface HistoryState {
  entries: HistoryEntry[];
  loading: boolean;
  /** Manually refetch the history list. */
  refresh: () => Promise<void>;
  /** Clear the persisted history, then refetch. */
  clear: () => Promise<void>;
}

/**
 * Polls the persisted transcription history for the History panel.
 *
 * Fetches immediately on mount, then re-fetches every ~3s while `active` is
 * true. Polling stops (and the interval is cleared) whenever `active` is false
 * or the consumer unmounts, so we never poll while the section is hidden.
 */
export function useHistory(active: boolean): HistoryState {
  const [entries, setEntries] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);

  // Guard against overlapping fetches and post-unmount state updates.
  const inflightRef = useRef(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    if (inflightRef.current) return;
    inflightRef.current = true;
    setLoading(true);
    try {
      const next = await commands.getHistory();
      if (!mountedRef.current) return;
      setEntries(next);
    } catch {
      // Best-effort: keep the last good snapshot on transient failures.
    } finally {
      if (mountedRef.current) setLoading(false);
      inflightRef.current = false;
    }
  }, []);

  const clear = useCallback(async () => {
    try {
      await commands.clearHistory();
    } catch {
      // Best-effort: still refresh so the UI reflects actual state.
    }
    await refresh();
  }, [refresh]);

  useEffect(() => {
    if (!active) return;
    void refresh();
    const id = window.setInterval(() => void refresh(), POLL_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [active, refresh]);

  return { entries, loading, refresh, clear };
}

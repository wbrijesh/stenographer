import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type Metrics } from "@/bindings";

const LOG_LINES = 200;
const POLL_INTERVAL_MS = 3000;

export interface DiagnosticsState {
  metrics: Metrics | null;
  logs: string[];
  loading: boolean;
  /** Manually refetch metrics + logs. */
  refresh: () => Promise<void>;
}

/**
 * Polls process metrics and recent log lines for the Diagnostics panel.
 *
 * Fetches immediately on mount, then re-fetches every ~3s while `active` is
 * true. Polling stops (and the interval is cleared) whenever `active` is false
 * or the consumer unmounts, so we never poll while the section is hidden.
 */
export function useDiagnostics(active: boolean): DiagnosticsState {
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
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
      const [nextMetrics, nextLogs] = await Promise.all([
        commands.getMetrics(),
        commands.getRecentLogs(LOG_LINES),
      ]);
      if (!mountedRef.current) return;
      setMetrics(nextMetrics);
      setLogs(nextLogs);
    } catch {
      // Best-effort: keep the last good snapshot on transient failures.
    } finally {
      if (mountedRef.current) setLoading(false);
      inflightRef.current = false;
    }
  }, []);

  useEffect(() => {
    if (!active) return;
    void refresh();
    const id = window.setInterval(() => void refresh(), POLL_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [active, refresh]);

  return { metrics, logs, loading, refresh };
}

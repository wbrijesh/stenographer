import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Section } from "@/components/ui";
import type { Metrics } from "@/bindings";
import { useDiagnostics } from "@/hooks/useDiagnostics";

interface Props {
  /** Whether the Diagnostics section is currently visible (drives polling). */
  active: boolean;
}

const EMPTY = "—";

/** Format a millisecond duration as `### ms` (sub-second) or `#.# s`. */
function formatMs(ms: number): string {
  if (!ms || ms <= 0) return EMPTY;
  if (ms < 1000) return `${Math.round(ms)} ms`;
  return `${(ms / 1000).toFixed(1)} s`;
}

/** Format an integer count, showing "—" for zero. */
function formatCount(n: number): string {
  if (!n || n <= 0) return EMPTY;
  return n.toLocaleString();
}

/** Format an elapsed-seconds duration as "1h 23m" / "5m 12s" / "42s". */
function formatUptime(seconds: number): string {
  if (!seconds || seconds <= 0) return EMPTY;
  const s = Math.floor(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${sec}s`;
  return `${sec}s`;
}

interface StatCardProps {
  label: string;
  value: string;
}

function StatCard({ label, value }: StatCardProps) {
  return (
    <div className="rounded-xl border border-black/10 bg-white/70 px-3.5 py-3 shadow-sm">
      <div className="text-[11px] font-medium uppercase tracking-wide text-black/45">
        {label}
      </div>
      <div className="mt-1 text-lg font-semibold tabular-nums tracking-tight">
        {value}
      </div>
    </div>
  );
}

interface MetricsGridProps {
  metrics: Metrics | null;
  uptimeSeconds: number;
}

function MetricsGrid({ metrics, uptimeSeconds }: MetricsGridProps) {
  const cards: StatCardProps[] = metrics
    ? [
        { label: "Transcriptions", value: formatCount(metrics.transcriptions) },
        {
          label: "Avg transcription",
          value: formatMs(metrics.avg_transcription_ms),
        },
        {
          label: "Last transcription",
          value: formatMs(metrics.last_transcription_ms),
        },
        { label: "AI cleanups", value: formatCount(metrics.cleanups) },
        { label: "Avg cleanup", value: formatMs(metrics.avg_cleanup_ms) },
        { label: "Last cleanup", value: formatMs(metrics.last_cleanup_ms) },
        { label: "Words dictated", value: formatCount(metrics.words_dictated) },
        { label: "Recordings", value: formatCount(metrics.recordings) },
        { label: "Cancelled", value: formatCount(metrics.cancelled) },
        { label: "Errors", value: formatCount(metrics.errors) },
        { label: "Uptime", value: formatUptime(uptimeSeconds) },
      ]
    : [];

  return (
    <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3">
      {cards.map((c) => (
        <StatCard key={c.label} label={c.label} value={c.value} />
      ))}
    </div>
  );
}

/** Classify a log line so we can tint errors/warnings. */
function logTone(line: string): "error" | "warn" | "normal" {
  if (/\b(ERROR|FATAL)\b/.test(line)) return "error";
  if (/\bWARN(ING)?\b/.test(line)) return "warn";
  return "normal";
}

/** Split a leading `[timestamp]` / `2026-...` prefix so it can be dimmed. */
function splitTimestamp(line: string): [string, string] {
  // Bracketed prefix, e.g. "[2026-06-21][INFO] ...".
  const bracket = line.match(/^(\[[^\]]+\])/);
  if (bracket) return [bracket[1], line.slice(bracket[1].length)];
  // ISO-ish leading date/time token.
  const iso = line.match(/^(\d{4}-\d{2}-\d{2}[T ][\d:.]+Z?)/);
  if (iso) return [iso[1], line.slice(iso[1].length)];
  return ["", line];
}

interface LogViewerProps {
  logs: string[];
}

function LogViewer({ logs }: LogViewerProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to the bottom (newest line) whenever the log set changes.
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [logs]);

  return (
    <div
      ref={scrollRef}
      className="h-64 select-text overflow-auto rounded-xl border border-black/10 bg-neutral-900 p-3 font-mono text-[11px] leading-relaxed text-neutral-200 shadow-inner"
    >
      {logs.length === 0 ? (
        <div className="text-neutral-500">No log lines available.</div>
      ) : (
        logs.map((line, i) => {
          const tone = logTone(line);
          const [ts, rest] = splitTimestamp(line);
          const toneClass =
            tone === "error"
              ? "text-red-400"
              : tone === "warn"
                ? "text-amber-400"
                : "text-neutral-200";
          return (
            <div
              key={i}
              className={`whitespace-pre-wrap break-words ${toneClass}`}
            >
              {ts && <span className="text-neutral-500">{ts}</span>}
              {rest}
            </div>
          );
        })
      )}
    </div>
  );
}

export function DiagnosticsSettings({ active }: Props) {
  const { metrics, logs, loading, refresh } = useDiagnostics(active);

  // Tick once a second so the uptime card stays live between 3s polls.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [active]);

  const uptimeSeconds = useMemo(() => {
    if (!metrics || !metrics.session_started_unix) return 0;
    return Math.max(0, Math.floor(now / 1000) - metrics.session_started_unix);
  }, [metrics, now]);

  return (
    <div className="space-y-6">
      <Section
        title="Metrics"
        description="Live pipeline statistics for this session. Auto-refreshes every few seconds."
      >
        <div className="space-y-3 px-4 py-3">
          <div className="flex justify-end">
            <Button
              size="sm"
              variant="secondary"
              disabled={loading}
              onClick={() => void refresh()}
            >
              {loading ? "Refreshing…" : "Refresh"}
            </Button>
          </div>
          <MetricsGrid metrics={metrics} uptimeSeconds={uptimeSeconds} />
        </div>
      </Section>

      <Section
        title="Recent logs"
        description="The most recent log lines, newest at the bottom."
      >
        <div className="space-y-3 px-4 py-3">
          <div className="flex items-center justify-between">
            <span className="text-xs text-black/45">
              {logs.length} {logs.length === 1 ? "line" : "lines"}
            </span>
            <Button
              size="sm"
              variant="secondary"
              disabled={loading}
              onClick={() => void refresh()}
            >
              {loading ? "Refreshing…" : "Refresh"}
            </Button>
          </div>
          <LogViewer logs={logs} />
        </div>
      </Section>
    </div>
  );
}

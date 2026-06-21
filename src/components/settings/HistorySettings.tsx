import { useEffect, useRef, useState } from "react";
import { Button, Section } from "@/components/ui";
import { commands, type HistoryEntry } from "@/bindings";
import { useHistory } from "@/hooks/useHistory";

interface Props {
  /** Whether the History section is currently visible (drives polling). */
  active: boolean;
}

/** Format a Unix-seconds timestamp as a coarse relative time ("2m ago"). */
function formatRelative(timestampUnix: number, nowMs: number): string {
  const diffSec = Math.max(0, Math.floor(nowMs / 1000) - timestampUnix);
  if (diffSec < 5) return "just now";
  if (diffSec < 60) return `${diffSec}s ago`;
  const min = Math.floor(diffSec / 60);
  if (min < 60) return `${min}m ago`;
  const hours = Math.floor(min / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

interface HistoryRowProps {
  entry: HistoryEntry;
  nowMs: number;
}

function HistoryRow({ entry, nowMs }: HistoryRowProps) {
  const [copied, setCopied] = useState(false);
  const timeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
    };
  }, []);

  async function handleCopy() {
    const result = await commands.copyText(entry.text);
    if (result.status !== "ok") return; // don't crash on backend error
    setCopied(true);
    if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
    timeoutRef.current = window.setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div className="flex items-start justify-between gap-3 px-3.5 py-2.5">
      <div className="min-w-0 flex-1">
        <p
          title={entry.text}
          data-selectable
          className="text-label line-clamp-3 text-[13px] leading-snug"
        >
          {entry.text}
        </p>
        <p className="text-tertiary mt-1 text-[11px] tabular-nums">
          {formatRelative(entry.timestamp_unix, nowMs)}
        </p>
      </div>
      <div className="shrink-0">
        <Button
          size="sm"
          variant="secondary"
          onClick={() => void handleCopy()}
        >
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </div>
  );
}

export function HistorySettings({ active }: Props) {
  const { entries, loading, refresh, clear } = useHistory(active);

  // Tick once a second so relative timestamps stay fresh between 3s polls.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [active]);

  return (
    <div className="space-y-6">
      <Section
        title="Recent transcriptions"
        description="Your last 10 transcriptions. Click Copy to put one back on the clipboard."
      >
        <div className="space-y-3 px-4 py-3">
          <div className="flex items-center justify-between">
            <span className="text-secondary text-[12px]">
              {entries.length}{" "}
              {entries.length === 1 ? "transcription" : "transcriptions"}
            </span>
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="secondary"
                disabled={loading}
                onClick={() => void refresh()}
              >
                {loading ? "Refreshing…" : "Refresh"}
              </Button>
              <Button
                size="sm"
                variant="danger"
                disabled={entries.length === 0}
                onClick={() => void clear()}
              >
                Clear history
              </Button>
            </div>
          </div>

          {entries.length === 0 ? (
            <div className="text-tertiary py-8 text-center text-[13px]">
              No transcriptions yet.
            </div>
          ) : (
            <div className="mac-card">
              {entries.map((entry, i) => (
                <div key={`${entry.timestamp_unix}-${i}`}>
                  {i > 0 && <div className="mac-divider" />}
                  <HistoryRow entry={entry} nowMs={now} />
                </div>
              ))}
            </div>
          )}
        </div>
      </Section>
    </div>
  );
}

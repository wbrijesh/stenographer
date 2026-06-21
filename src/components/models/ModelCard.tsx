import { Button, ProgressBar } from "@/components/ui";
import { CheckIcon, DownloadIcon, TrashIcon } from "@/components/icons";
import type { DownloadProgress, ModelInfo } from "@/lib/modelApi";
import { ScoreBar } from "./ScoreBar";

interface ModelCardProps {
  model: ModelInfo;
  isSelected: boolean;
  progress?: DownloadProgress;
  /** MB/s */
  speed?: number;
  onSelect: (id: string) => void;
  onDownload: (id: string) => void;
  onCancel: (id: string) => void;
  onDelete: (id: string) => void;
}

function formatSize(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb} MB`;
}

/** Map a backend 0.0–1.0 score to the ScoreBar's 1–5 pip scale. */
function toPips(score: number): number {
  return Math.max(1, Math.min(5, Math.round(score * 5)));
}

export function ModelCard({
  model,
  isSelected,
  progress,
  speed,
  onSelect,
  onDownload,
  onCancel,
  onDelete,
}: ModelCardProps) {
  const downloading = model.is_downloading;

  return (
    <div
      className="mac-card p-4 transition-colors"
      style={
        isSelected
          ? {
              borderColor: "color-mix(in srgb, var(--accent) 55%, transparent)",
              background: "color-mix(in srgb, var(--accent) 7%, var(--card-bg))",
            }
          : undefined
      }
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-label text-[13px] font-semibold">
              {model.name}
            </h3>
            {model.is_recommended && (
              <span className="text-accent rounded-full bg-[color-mix(in_srgb,var(--accent)_12%,transparent)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide">
                Recommended
              </span>
            )}
            {isSelected && (
              <span
                className="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide"
                style={{
                  background: "var(--accent)",
                  color: "var(--accent-fg)",
                }}
              >
                <span className="h-3 w-3">
                  <CheckIcon />
                </span>
                Active
              </span>
            )}
          </div>
          <p className="text-secondary mt-1 text-[12px] leading-snug">
            {model.description}
          </p>
        </div>
        <span className="text-secondary shrink-0 rounded-md bg-[var(--fill)] px-2 py-1 text-[12px] font-medium tabular-nums">
          {formatSize(model.size_mb)}
        </span>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-1.5">
        <ScoreBar label="Accuracy" score={toPips(model.accuracy_score)} />
        <ScoreBar label="Speed" score={toPips(model.speed_score)} />
        <span className="text-tertiary rounded bg-[var(--fill)] px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide">
          {model.engine_type}
        </span>
        {model.supports_translation && (
          <span className="text-tertiary text-[10px] font-medium uppercase tracking-wide">
            Translation
          </span>
        )}
      </div>

      {downloading && (
        <div className="mt-3 space-y-1.5">
          <ProgressBar percentage={Math.round(progress?.percentage ?? 0)} />
          <div className="text-secondary flex items-center justify-between text-[11px] tabular-nums">
            <span>{Math.round(progress?.percentage ?? 0)}%</span>
            <span>
              {speed && speed > 0 ? `${speed.toFixed(1)} MB/s` : "Downloading…"}
            </span>
          </div>
        </div>
      )}

      <div className="mt-3 flex items-center justify-end gap-2">
        {downloading ? (
          <Button
            variant="secondary"
            size="sm"
            onClick={() => onCancel(model.id)}
          >
            Cancel
          </Button>
        ) : model.is_downloaded ? (
          <>
            <Button
              variant="ghost"
              size="sm"
              aria-label="Delete model"
              onClick={() => onDelete(model.id)}
            >
              <span className="h-3.5 w-3.5">
                <TrashIcon />
              </span>
              Delete
            </Button>
            <Button
              variant={isSelected ? "secondary" : "primary"}
              size="sm"
              disabled={isSelected}
              onClick={() => onSelect(model.id)}
            >
              {isSelected ? "Selected" : "Use this model"}
            </Button>
          </>
        ) : (
          <Button
            variant="primary"
            size="sm"
            onClick={() => onDownload(model.id)}
          >
            <span className="h-3.5 w-3.5">
              <DownloadIcon />
            </span>
            Download
          </Button>
        )}
      </div>
    </div>
  );
}

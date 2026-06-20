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
      className={`rounded-xl border p-4 shadow-sm transition-colors ${
        isSelected
          ? "border-blue-500/60 bg-blue-500/[0.04] ring-1 ring-blue-500/30"
          : "border-black/10 bg-white/70"
      }`}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-sm font-semibold">{model.name}</h3>
            {model.is_recommended && (
              <span className="rounded-full bg-blue-500/10 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-blue-600">
                Recommended
              </span>
            )}
            {isSelected && (
              <span className="inline-flex items-center gap-1 rounded-full bg-blue-500 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-white">
                <span className="h-3 w-3">
                  <CheckIcon />
                </span>
                Active
              </span>
            )}
          </div>
          <p className="mt-1 text-xs leading-snug text-black/55">
            {model.description}
          </p>
        </div>
        <span className="shrink-0 rounded-md bg-black/[0.05] px-2 py-1 text-xs font-medium tabular-nums text-black/60">
          {formatSize(model.size_mb)}
        </span>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-1.5">
        <ScoreBar label="Accuracy" score={model.accuracy_score} />
        <ScoreBar label="Speed" score={model.speed_score} />
        <span className="rounded bg-black/[0.05] px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-black/45">
          {model.engine_type}
        </span>
        {model.supports_translation && (
          <span className="text-[10px] font-medium uppercase tracking-wide text-black/40">
            Translation
          </span>
        )}
      </div>

      {downloading && (
        <div className="mt-3 space-y-1.5">
          <ProgressBar percentage={progress?.percentage ?? 0} />
          <div className="flex items-center justify-between text-[11px] tabular-nums text-black/50">
            <span>{progress?.percentage ?? 0}%</span>
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

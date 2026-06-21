import { useEffect, useMemo, useState } from "react";
import { commands, type ModelInfo } from "@/bindings";
import { useModelStore } from "@/stores/modelStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { Button, ProgressBar } from "@/components/ui";
import { DownloadIcon, ModelsIcon } from "@/components/icons";
import { GrantedBadge, StepShell } from "./StepShell";

interface ModelStepProps {
  /** Called once a model is downloaded AND selected as the active model. */
  onReady: () => void;
}

function formatSize(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb} MB`;
}

/**
 * Step 5 — Model. Reuses {@link useModelStore} for the catalog, download action,
 * and live progress events. The recommended model (`is_recommended`) is offered
 * as the one-tap default; the rest of the catalog can be expanded. When the
 * chosen model finishes downloading we persist it via `setSelectedModel`
 * (backed by `changeSelectedModel`) and advance the gate.
 */
export function ModelStep({ onReady }: ModelStepProps) {
  const {
    models,
    loading,
    error,
    downloadProgress,
    downloadStats,
    initialize,
    download,
    cancel,
  } = useModelStore();
  const { setSelectedModel } = useSettingsStore();

  // The model id the user opted to install during onboarding (drives the
  // "download finished → select it" effect).
  const [installingId, setInstallingId] = useState<string | null>(null);
  const [selecting, setSelecting] = useState(false);
  const [showCatalog, setShowCatalog] = useState(false);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const recommended = useMemo(
    () => models.find((m) => m.is_recommended) ?? null,
    [models],
  );

  const sortedCatalog = useMemo(
    () => [...models].sort((a, b) => a.size_mb - b.size_mb),
    [models],
  );

  // Once the model we're installing reports downloaded, select + advance.
  useEffect(() => {
    if (!installingId || selecting) return;
    const model = models.find((m) => m.id === installingId);
    if (model?.is_downloaded && !model.is_downloading) {
      setSelecting(true);
      void (async () => {
        const result = await commands.changeSelectedModel(installingId);
        if (result.status === "ok") {
          // Keep the settings store in sync so the rest of the app sees it too.
          await setSelectedModel(installingId);
          onReady();
        }
        setSelecting(false);
      })();
    }
  }, [installingId, selecting, models, setSelectedModel, onReady]);

  const startDownload = (model: ModelInfo) => {
    setInstallingId(model.id);
    if (model.is_downloaded) return; // effect above will select it.
    void download(model.id);
  };

  const renderInstallRow = (model: ModelInfo) => {
    const downloading = model.is_downloading;
    const progress = downloadProgress[model.id];
    const speed = downloadStats[model.id]?.speed;
    const isInstalling = installingId === model.id;

    return (
      <div key={model.id} className="mac-card p-3 text-left">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-label text-[13px] font-semibold">
                {model.name}
              </span>
              {model.is_recommended && (
                <span className="text-accent rounded-full bg-[color-mix(in_srgb,var(--accent)_12%,transparent)] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide">
                  Recommended
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

        {downloading && (
          <div className="mt-3 space-y-1.5">
            <ProgressBar percentage={Math.round(progress?.percentage ?? 0)} />
            <div className="text-secondary flex items-center justify-between text-[11px] tabular-nums">
              <span>{Math.round(progress?.percentage ?? 0)}%</span>
              <span>
                {speed && speed > 0
                  ? `${speed.toFixed(1)} MB/s`
                  : "Downloading…"}
              </span>
            </div>
          </div>
        )}

        <div className="mt-3 flex items-center justify-end gap-2">
          {downloading ? (
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                if (installingId === model.id) setInstallingId(null);
                void cancel(model.id);
              }}
            >
              Cancel
            </Button>
          ) : isInstalling && selecting ? (
            <Button variant="primary" size="sm" disabled>
              Selecting…
            </Button>
          ) : (
            <Button
              variant="primary"
              size="sm"
              disabled={installingId !== null}
              onClick={() => startDownload(model)}
            >
              <span className="h-3.5 w-3.5">
                <DownloadIcon />
              </span>
              {model.is_downloaded ? "Use this model" : "Download"}
            </Button>
          )}
        </div>
      </div>
    );
  };

  return (
    <StepShell
      icon={<ModelsIcon />}
      title="Download a speech model"
      description="Transcription runs entirely on-device using a downloaded model. The recommended one is a good balance of speed and accuracy."
    >
      <div className="space-y-3">
        {error && (
          <div className="text-red rounded-lg border border-[color-mix(in_srgb,var(--red)_30%,transparent)] bg-[color-mix(in_srgb,var(--red)_8%,transparent)] px-3 py-2 text-left text-[12px]">
            {error}
          </div>
        )}

        {loading && models.length === 0 ? (
          <p className="text-secondary text-[13px]">Loading models…</p>
        ) : (
          <>
            {recommended && renderInstallRow(recommended)}

            {sortedCatalog.length > (recommended ? 1 : 0) && (
              <button
                type="button"
                className="text-accent text-[12px] hover:underline"
                onClick={() => setShowCatalog((v) => !v)}
              >
                {showCatalog
                  ? "Hide other models"
                  : "Choose a different model"}
              </button>
            )}

            {showCatalog && (
              <div className="space-y-3">
                {sortedCatalog
                  .filter((m) => m.id !== recommended?.id)
                  .map(renderInstallRow)}
              </div>
            )}

            {!recommended && sortedCatalog.length === 0 && (
              <p className="text-secondary text-[13px]">No models available.</p>
            )}
          </>
        )}

        <p className="text-tertiary text-[11px]">
          You can add or switch models any time from Settings → Models.
        </p>
      </div>
    </StepShell>
  );
}

/** Compact confirmation reused by the gate when a model is already set. */
export function ModelGranted() {
  return (
    <div className="flex justify-center">
      <GrantedBadge label="Model ready" />
    </div>
  );
}

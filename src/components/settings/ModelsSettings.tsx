import { useEffect } from "react";
import type { AppSettings } from "@/bindings";
import { useModelStore } from "@/stores/modelStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { modelApi } from "@/lib/modelApi";
import { ModelCard } from "@/components/models/ModelCard";

interface Props {
  settings: AppSettings;
}

export function ModelsSettings({ settings }: Props) {
  const {
    models,
    loading,
    error,
    downloadProgress,
    downloadStats,
    initialize,
    download,
    cancel,
    remove,
  } = useModelStore();
  const { setSelectedModel } = useSettingsStore();

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const selectedModel = settings.selected_model ?? null;

  return (
    <div className="space-y-4">
      <div className="px-1">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-black/45">
          Speech models
        </h2>
        <p className="mt-0.5 text-xs text-black/45">
          Download a model and pick the one to use for transcription.
        </p>
        {modelApi.isMock && (
          <p className="mt-1 inline-block rounded bg-amber-400/15 px-2 py-0.5 text-[11px] font-medium text-amber-700">
            Mock data — backend model commands are not wired yet.
          </p>
        )}
      </div>

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/[0.06] px-3 py-2 text-xs text-red-700">
          {error}
        </div>
      )}

      {loading && models.length === 0 ? (
        <p className="px-1 text-sm text-black/50">Loading models…</p>
      ) : (
        <div className="space-y-3">
          {models.map((model) => (
            <ModelCard
              key={model.id}
              model={model}
              isSelected={selectedModel === model.id}
              progress={downloadProgress[model.id]}
              speed={downloadStats[model.id]?.speed}
              onSelect={(id) => void setSelectedModel(id)}
              onDownload={(id) => void download(id)}
              onCancel={(id) => void cancel(id)}
              onDelete={(id) => {
                if (selectedModel === id) void setSelectedModel(null);
                void remove(id);
              }}
            />
          ))}
        </div>
      )}
    </div>
  );
}

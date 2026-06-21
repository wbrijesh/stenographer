import { useEffect } from "react";
import type { AppSettings } from "@/bindings";
import { useModelStore } from "@/stores/modelStore";
import { useSettingsStore } from "@/stores/settingsStore";
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
    <div className="space-y-2">
      <h2 className="mac-group-header">Speech models</h2>
      <p className="mac-group-desc !pb-1">
        Download a model and pick the one to use for transcription.
      </p>

      {error && (
        <div className="text-red rounded-lg border border-[color-mix(in_srgb,var(--red)_30%,transparent)] bg-[color-mix(in_srgb,var(--red)_8%,transparent)] px-3 py-2 text-[12px]">
          {error}
        </div>
      )}

      {loading && models.length === 0 ? (
        <p className="text-secondary text-[13px]">Loading models…</p>
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

import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import {
  modelApi,
  type DownloadProgress,
  type ModelInfo,
} from "@/lib/modelApi";

interface DownloadStats {
  startTime: number;
  lastUpdate: number;
  totalDownloaded: number;
  /** MB/s, smoothed. */
  speed: number;
}

interface ModelStoreState {
  models: ModelInfo[];
  loading: boolean;
  error: string | null;
  initialized: boolean;
  /** modelId -> progress */
  downloadProgress: Record<string, DownloadProgress>;
  /** modelId -> stats (speed) */
  downloadStats: Record<string, DownloadStats>;

  initialize: () => Promise<void>;
  loadModels: () => Promise<void>;
  download: (modelId: string) => Promise<void>;
  cancel: (modelId: string) => Promise<void>;
  remove: (modelId: string) => Promise<void>;
}

export const useModelStore = create<ModelStoreState>()(
  immer((set, get) => {
    let unsubscribers: Array<() => void> = [];

    return {
      models: [],
      loading: false,
      error: null,
      initialized: false,
      downloadProgress: {},
      downloadStats: {},

      loadModels: async () => {
        set((s) => {
          s.loading = true;
        });
        try {
          const models = await modelApi.getAvailableModels();
          set((s) => {
            s.models = models;
            s.loading = false;
            s.error = null;
          });
        } catch (e) {
          set((s) => {
            s.loading = false;
            s.error = e instanceof Error ? e.message : String(e);
          });
        }
      },

      initialize: async () => {
        if (get().initialized) return;
        set((s) => {
          s.initialized = true;
        });

        await get().loadModels();

        // Tear down any previous listeners (HMR safety).
        unsubscribers.forEach((u) => u());
        unsubscribers = [];

        unsubscribers.push(
          modelApi.subscribeDownloadProgress((progress) => {
            set((s) => {
              s.downloadProgress[progress.model_id] = progress;

              // Mark the model as downloading optimistically.
              const m = s.models.find((x) => x.id === progress.model_id);
              if (m) m.is_downloading = true;

              // Speed calc.
              const now = Date.now();
              const current = s.downloadStats[progress.model_id];
              if (!current) {
                s.downloadStats[progress.model_id] = {
                  startTime: now,
                  lastUpdate: now,
                  totalDownloaded: progress.downloaded,
                  speed: 0,
                };
              } else {
                const timeDiff = (now - current.lastUpdate) / 1000;
                const bytesDiff = progress.downloaded - current.totalDownloaded;
                if (timeDiff > 0.4) {
                  const instant = bytesDiff / (1024 * 1024) / timeDiff;
                  const valid = Math.max(0, instant);
                  const smoothed =
                    current.speed > 0
                      ? current.speed * 0.7 + valid * 0.3
                      : valid;
                  current.lastUpdate = now;
                  current.totalDownloaded = progress.downloaded;
                  current.speed = Math.max(0, smoothed);
                }
              }
            });
          }),
        );

        unsubscribers.push(
          modelApi.subscribeDownloadComplete((modelId) => {
            set((s) => {
              delete s.downloadProgress[modelId];
              delete s.downloadStats[modelId];
              const m = s.models.find((x) => x.id === modelId);
              if (m) {
                m.is_downloading = false;
                m.is_downloaded = true;
              }
            });
          }),
        );

        unsubscribers.push(
          modelApi.subscribeDownloadFailed((modelId, error) => {
            set((s) => {
              delete s.downloadProgress[modelId];
              delete s.downloadStats[modelId];
              const m = s.models.find((x) => x.id === modelId);
              if (m) m.is_downloading = false;
              s.error = `Download failed for ${modelId}: ${error}`;
            });
          }),
        );
      },

      download: async (modelId) => {
        set((s) => {
          s.error = null;
          const m = s.models.find((x) => x.id === modelId);
          if (m) m.is_downloading = true;
          s.downloadProgress[modelId] = {
            model_id: modelId,
            downloaded: 0,
            total: 0,
            percentage: 0,
          };
        });
        try {
          await modelApi.downloadModel(modelId);
        } catch (e) {
          set((s) => {
            delete s.downloadProgress[modelId];
            delete s.downloadStats[modelId];
            const m = s.models.find((x) => x.id === modelId);
            if (m) m.is_downloading = false;
            s.error = e instanceof Error ? e.message : String(e);
          });
        }
      },

      cancel: async (modelId) => {
        try {
          await modelApi.cancelDownload(modelId);
        } finally {
          set((s) => {
            delete s.downloadProgress[modelId];
            delete s.downloadStats[modelId];
            const m = s.models.find((x) => x.id === modelId);
            if (m) m.is_downloading = false;
          });
        }
      },

      remove: async (modelId) => {
        try {
          await modelApi.deleteModel(modelId);
          set((s) => {
            const m = s.models.find((x) => x.id === modelId);
            if (m) {
              m.is_downloaded = false;
              m.is_downloading = false;
            }
          });
        } catch (e) {
          set((s) => {
            s.error = e instanceof Error ? e.message : String(e);
          });
        }
      },
    };
  }),
);

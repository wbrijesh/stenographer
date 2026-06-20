/**
 * Model API abstraction layer.
 *
 * The backend model module (get_available_models / download_model / ...) is being
 * built in PARALLEL and is NOT yet present in `src/bindings.ts`. To keep the Models
 * UI fully functional today, this file provides a MOCK implementation behind a typed
 * interface that mirrors the expected real contract.
 *
 * SWAP INSTRUCTIONS (for the integration layer / orchestrator):
 *   Once the backend exposes the model commands + events, replace the `modelApi`
 *   object below with thin wrappers over the generated `commands.*` calls and have
 *   `subscribeDownloadProgress` forward the real Tauri `model-download-progress`
 *   event. The rest of the app imports ONLY `modelApi` and the types from this file,
 *   so nothing else needs to change.
 *
 * Expected real bindings (NOT yet in bindings.ts — orchestrator must add):
 *   - commands.getAvailableModels(): Promise<ModelInfo[]>
 *   - commands.downloadModel(id: string): Promise<Result<null, string>>
 *   - commands.cancelDownload(id: string): Promise<Result<null, string>>
 *   - commands.deleteModel(id: string): Promise<Result<null, string>>
 *   - event "model-download-progress": { model_id, downloaded, total, percentage }
 *   - event "model-download-complete": string (model_id)
 *   - event "model-download-failed": { model_id, error }
 *   Selecting the active model already exists: commands.changeSelectedModel(id).
 */

export type ModelEngineType = "whisper" | "parakeet" | string;

/** Mirrors the expected backend `ModelInfo` contract. */
export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  size_mb: number;
  is_downloaded: boolean;
  is_downloading: boolean;
  /** 1-5 relative accuracy score. */
  accuracy_score: number;
  /** 1-5 relative speed score. */
  speed_score: number;
  is_recommended: boolean;
  supports_translation: boolean;
  engine_type: ModelEngineType;
}

/** Payload of the `model-download-progress` event. */
export interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

export type DownloadProgressHandler = (progress: DownloadProgress) => void;
export type DownloadCompleteHandler = (modelId: string) => void;
export type DownloadFailedHandler = (modelId: string, error: string) => void;

export interface ModelApi {
  getAvailableModels(): Promise<ModelInfo[]>;
  downloadModel(modelId: string): Promise<void>;
  cancelDownload(modelId: string): Promise<void>;
  deleteModel(modelId: string): Promise<void>;
  /** Returns an unsubscribe function. */
  subscribeDownloadProgress(handler: DownloadProgressHandler): () => void;
  subscribeDownloadComplete(handler: DownloadCompleteHandler): () => void;
  subscribeDownloadFailed(handler: DownloadFailedHandler): () => void;
  /** True when this is the real (backend-backed) implementation. */
  readonly isMock: boolean;
}

// ---------------------------------------------------------------------------
// MOCK IMPLEMENTATION
// ---------------------------------------------------------------------------

const MOCK_MODELS: ModelInfo[] = [
  {
    id: "whisper-tiny",
    name: "Whisper Tiny",
    description: "Fastest, lowest accuracy. Great for quick notes on any Mac.",
    size_mb: 75,
    is_downloaded: true,
    is_downloading: false,
    accuracy_score: 2,
    speed_score: 5,
    is_recommended: false,
    supports_translation: true,
    engine_type: "whisper",
  },
  {
    id: "whisper-base",
    name: "Whisper Base",
    description: "Balanced speed and accuracy for everyday dictation.",
    size_mb: 142,
    is_downloaded: false,
    is_downloading: false,
    accuracy_score: 3,
    speed_score: 4,
    is_recommended: true,
    supports_translation: true,
    engine_type: "whisper",
  },
  {
    id: "whisper-small",
    name: "Whisper Small",
    description: "Higher accuracy, noticeably slower. Good for clean transcripts.",
    size_mb: 466,
    is_downloaded: false,
    is_downloading: false,
    accuracy_score: 4,
    speed_score: 3,
    is_recommended: false,
    supports_translation: true,
    engine_type: "whisper",
  },
  {
    id: "whisper-large-v3-turbo",
    name: "Whisper Large v3 Turbo",
    description: "Best accuracy with optimized speed. Needs Apple Silicon.",
    size_mb: 1620,
    is_downloaded: false,
    is_downloading: false,
    accuracy_score: 5,
    speed_score: 3,
    is_recommended: false,
    supports_translation: true,
    engine_type: "whisper",
  },
  {
    id: "parakeet-v2",
    name: "Parakeet v2",
    description: "Ultra-fast English-only model. No translation support.",
    size_mb: 620,
    is_downloaded: false,
    is_downloading: false,
    accuracy_score: 4,
    speed_score: 5,
    is_recommended: false,
    supports_translation: false,
    engine_type: "parakeet",
  },
];

type Listener<T> = (arg: T) => void;

class MockModelApi implements ModelApi {
  readonly isMock = true;

  private models: ModelInfo[] = MOCK_MODELS.map((m) => ({ ...m }));
  private progressListeners = new Set<DownloadProgressHandler>();
  private completeListeners = new Set<DownloadCompleteHandler>();
  private failedListeners = new Set<DownloadFailedHandler>();
  private timers = new Map<string, ReturnType<typeof setInterval>>();

  async getAvailableModels(): Promise<ModelInfo[]> {
    // Simulate a small async load.
    await delay(120);
    return this.models.map((m) => ({ ...m }));
  }

  async downloadModel(modelId: string): Promise<void> {
    const model = this.models.find((m) => m.id === modelId);
    if (!model || model.is_downloaded || model.is_downloading) return;

    model.is_downloading = true;
    const total = model.size_mb * 1024 * 1024;
    let downloaded = 0;
    // ~3s download regardless of size, for a snappy mock.
    const stepBytes = total / 30;

    const timer = setInterval(() => {
      downloaded = Math.min(total, downloaded + stepBytes);
      const percentage = Math.round((downloaded / total) * 100);
      this.emit(this.progressListeners, {
        model_id: modelId,
        downloaded,
        total,
        percentage,
      });
      if (downloaded >= total) {
        clearInterval(timer);
        this.timers.delete(modelId);
        model.is_downloading = false;
        model.is_downloaded = true;
        this.emit(this.completeListeners, modelId);
      }
    }, 100);
    this.timers.set(modelId, timer);
  }

  async cancelDownload(modelId: string): Promise<void> {
    const timer = this.timers.get(modelId);
    if (timer) {
      clearInterval(timer);
      this.timers.delete(modelId);
    }
    const model = this.models.find((m) => m.id === modelId);
    if (model) model.is_downloading = false;
  }

  async deleteModel(modelId: string): Promise<void> {
    await delay(120);
    const model = this.models.find((m) => m.id === modelId);
    if (model) {
      model.is_downloaded = false;
      model.is_downloading = false;
    }
  }

  subscribeDownloadProgress(handler: DownloadProgressHandler): () => void {
    this.progressListeners.add(handler);
    return () => this.progressListeners.delete(handler);
  }

  subscribeDownloadComplete(handler: DownloadCompleteHandler): () => void {
    this.completeListeners.add(handler);
    return () => this.completeListeners.delete(handler);
  }

  subscribeDownloadFailed(handler: DownloadFailedHandler): () => void {
    this.failedListeners.add(handler);
    return () => this.failedListeners.delete(handler);
  }

  private emit<T>(listeners: Set<Listener<T>>, arg: T): void {
    listeners.forEach((l) => l(arg));
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** The single instance the app talks to. Swap this for a real impl later. */
export const modelApi: ModelApi = new MockModelApi();

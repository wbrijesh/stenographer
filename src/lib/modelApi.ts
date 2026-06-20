/**
 * Model API abstraction layer.
 *
 * Thin wrappers over the generated `commands.*` calls plus the real Tauri
 * download-lifecycle events. The rest of the app imports ONLY `modelApi` and the
 * re-exported `ModelInfo` type from this file.
 *
 * Backend reference (src-tauri/src/managers/model.rs + commands/models.rs):
 *   - commands.getAvailableModels(): Result<ModelInfo[], string>
 *   - commands.downloadModel(id): Result<null, string>
 *   - commands.cancelDownload(id): Result<null, string>
 *   - commands.deleteModel(id): Result<null, string>
 *   - event "model-download-progress": { id, downloaded, total, percentage }
 *   - event "model-download-complete": string (model id)
 *   - event "model-download-failed": { id, error }
 *   - event "model-download-cancelled": { id }
 */

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type ModelInfo, type Result } from "@/bindings";

export type { ModelInfo } from "@/bindings";

/**
 * Normalized download-progress shape used across the frontend. Mirrors the
 * backend `model-download-progress` payload, which emits the model id under the
 * `id` field.
 */
export interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

/** Raw backend `model-download-progress` event payload. */
interface RawDownloadProgress {
  id: string;
  downloaded: number;
  total: number;
  percentage: number;
}

/** Raw backend `model-download-failed` / `model-download-cancelled` payload. */
interface RawDownloadIdError {
  id: string;
  error?: string;
}

export type DownloadProgressHandler = (progress: DownloadProgress) => void;
export type DownloadCompleteHandler = (modelId: string) => void;
export type DownloadFailedHandler = (modelId: string, error: string) => void;
export type DownloadCancelledHandler = (modelId: string) => void;

export interface ModelApi {
  getAvailableModels(): Promise<ModelInfo[]>;
  downloadModel(modelId: string): Promise<void>;
  cancelDownload(modelId: string): Promise<void>;
  deleteModel(modelId: string): Promise<void>;
  /** Returns an unsubscribe function. */
  subscribeDownloadProgress(handler: DownloadProgressHandler): () => void;
  subscribeDownloadComplete(handler: DownloadCompleteHandler): () => void;
  subscribeDownloadFailed(handler: DownloadFailedHandler): () => void;
  subscribeDownloadCancelled(handler: DownloadCancelledHandler): () => void;
}

/** Unwrap a tauri-specta `Result` envelope, throwing on the error variant. */
function unwrap<T>(result: Result<T, string>): T {
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

/**
 * Subscribe to a Tauri event. `listen` resolves asynchronously, so we return a
 * synchronous unsubscribe that tears down the listener once it's registered (or
 * immediately marks it for teardown if unsubscribed before that).
 */
function subscribe<P>(
  event: string,
  handler: (payload: P) => void,
): () => void {
  let unlisten: UnlistenFn | null = null;
  let cancelled = false;

  void listen<P>(event, (e) => handler(e.payload)).then((fn) => {
    if (cancelled) {
      fn();
    } else {
      unlisten = fn;
    }
  });

  return () => {
    cancelled = true;
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  };
}

class RealModelApi implements ModelApi {
  async getAvailableModels(): Promise<ModelInfo[]> {
    return unwrap(await commands.getAvailableModels());
  }

  async downloadModel(modelId: string): Promise<void> {
    unwrap(await commands.downloadModel(modelId));
  }

  async cancelDownload(modelId: string): Promise<void> {
    unwrap(await commands.cancelDownload(modelId));
  }

  async deleteModel(modelId: string): Promise<void> {
    unwrap(await commands.deleteModel(modelId));
  }

  subscribeDownloadProgress(handler: DownloadProgressHandler): () => void {
    return subscribe<RawDownloadProgress>("model-download-progress", (p) => {
      handler({
        model_id: p.id,
        downloaded: p.downloaded,
        total: p.total,
        percentage: p.percentage,
      });
    });
  }

  subscribeDownloadComplete(handler: DownloadCompleteHandler): () => void {
    return subscribe<string>("model-download-complete", (modelId) => {
      handler(modelId);
    });
  }

  subscribeDownloadFailed(handler: DownloadFailedHandler): () => void {
    return subscribe<RawDownloadIdError>("model-download-failed", (p) => {
      handler(p.id, p.error ?? "Download failed");
    });
  }

  subscribeDownloadCancelled(handler: DownloadCancelledHandler): () => void {
    return subscribe<RawDownloadIdError>("model-download-cancelled", (p) => {
      handler(p.id);
    });
  }
}

/** The single instance the app talks to. */
export const modelApi: ModelApi = new RealModelApi();

import { useCallback, useEffect, useState } from "react";
import { commands } from "@/bindings";

/**
 * Live prerequisite state the first-run gate is derived from.
 *
 * The app is only usable once macOS Microphone + Accessibility are granted and
 * at least one model is downloaded AND selected. There is no persisted
 * "onboarding completed" flag — completion is derived purely from these live
 * checks, so the gate never shows again once everything is satisfied (and
 * reappears if a prerequisite is later revoked).
 */
export interface OnboardingStatus {
  /** Microphone permission granted. */
  microphone: boolean;
  /** Accessibility (AX) permission granted. */
  accessibility: boolean;
  /** A model is downloaded and persisted as the selected model. */
  model: boolean;
  /** True once the initial checks have resolved (avoids a flash of onboarding). */
  ready: boolean;
}

const INITIAL: OnboardingStatus = {
  microphone: false,
  accessibility: false,
  model: false,
  ready: false,
};

/** All hard prerequisites met (Globe-key guidance is non-fatal, excluded here). */
export function prerequisitesMet(status: OnboardingStatus): boolean {
  return status.microphone && status.accessibility && status.model;
}

/** Whether a model is both downloaded and selected as the active model. */
async function checkModelReady(): Promise<boolean> {
  const [modelsResult, settings] = await Promise.all([
    commands.getAvailableModels(),
    commands.getAppSettings(),
  ]);
  if (modelsResult.status === "error") return false;
  const selected = settings.selected_model ?? null;
  if (!selected) return false;
  return modelsResult.data.some((m) => m.id === selected && m.is_downloaded);
}

/** Result of the first-run gate hook. */
export interface UseOnboardingResult {
  /** Live prerequisite snapshot. */
  status: OnboardingStatus;
  /** Re-run all prerequisite checks (steps call this as they make progress). */
  recheck: () => Promise<void>;
  /** Alias of {@link recheck}, kept for call sites that say `refresh`. */
  refresh: () => Promise<void>;
  /** True once the initial checks have resolved. */
  ready: boolean;
  /** True when the gate should be shown (a hard prerequisite is unmet). */
  needsOnboarding: boolean;
}

/**
 * Drives the first-run gate. Runs the three prerequisite checks on mount and
 * exposes a `recheck` so individual onboarding steps can re-evaluate the gate
 * as they make progress (e.g. after a model finishes downloading + is selected).
 *
 * `needsOnboarding` is only meaningful once `ready` is true; while still loading
 * it is `false` so callers can render a neutral spinner instead of flashing the
 * onboarding UI.
 */
export function useOnboarding(): UseOnboardingResult {
  const [status, setStatus] = useState<OnboardingStatus>(INITIAL);

  const recheck = useCallback(async () => {
    const [microphone, accessibility, model] = await Promise.all([
      commands.checkMicrophonePermission(),
      commands.checkAccessibilityPermission(),
      checkModelReady(),
    ]);
    setStatus({ microphone, accessibility, model, ready: true });
  }, []);

  useEffect(() => {
    void recheck();
  }, [recheck]);

  return {
    status,
    recheck,
    refresh: recheck,
    ready: status.ready,
    needsOnboarding: status.ready && !prerequisitesMet(status),
  };
}

/**
 * Poll a boolean `check*` permission command until it returns true or the
 * attempt budget is exhausted. macOS `request*` calls are fire-and-forget and
 * resolve before the user actually grants access, so polling the matching
 * `check*` is the only reliable signal.
 *
 * @returns `true` if the permission became granted within the budget.
 */
export async function pollPermission(
  check: () => Promise<boolean>,
  opts: { intervalMs?: number; maxAttempts?: number; signal?: AbortSignal } = {},
): Promise<boolean> {
  const { intervalMs = 1000, maxAttempts = 60, signal } = opts;
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    if (signal?.aborted) return false;
    if (await check()) return true;
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
  return check();
}

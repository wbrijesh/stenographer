import { useEffect } from "react";
import { useSettingsStore } from "@/stores/settingsStore";
import "./App.css";

function App() {
  const { settings, loading, initialize, setPasteDelayMs, setOverlayEnabled } =
    useSettingsStore();

  useEffect(() => {
    void initialize();
  }, [initialize]);

  return (
    <main className="min-h-screen p-8">
      <h1 className="text-2xl font-semibold">Stenographer</h1>
      <p className="mt-1 text-sm opacity-60">Phase 0 scaffold</p>

      {loading && <p className="mt-4">Loading settings…</p>}

      {settings && (
        <section className="mt-6 space-y-4">
          <div className="flex items-center gap-3">
            <label htmlFor="paste-delay" className="w-40">
              Paste delay (ms)
            </label>
            <input
              id="paste-delay"
              type="number"
              className="w-24 rounded border px-2 py-1"
              value={settings.paste_delay_ms}
              onChange={(e) => void setPasteDelayMs(Number(e.target.value))}
            />
          </div>

          <div className="flex items-center gap-3">
            <label htmlFor="overlay-enabled" className="w-40">
              Overlay enabled
            </label>
            <input
              id="overlay-enabled"
              type="checkbox"
              checked={settings.overlay_enabled}
              onChange={(e) => void setOverlayEnabled(e.target.checked)}
            />
          </div>

          <details className="mt-6">
            <summary className="cursor-pointer text-sm opacity-70">
              Raw settings
            </summary>
            <pre className="mt-2 overflow-auto rounded bg-black/5 p-3 text-xs">
              {JSON.stringify(settings, null, 2)}
            </pre>
          </details>
        </section>
      )}
    </main>
  );
}

export default App;

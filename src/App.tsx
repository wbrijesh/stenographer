import { useEffect, useState } from "react";
import { useSettingsStore } from "@/stores/settingsStore";
import { SidebarItem } from "@/components/ui";
import {
  AdvancedSettings,
  DiagnosticsSettings,
  GeneralSettings,
  ModelsSettings,
} from "@/components/settings";
import {
  AdvancedIcon,
  DiagnosticsIcon,
  GeneralIcon,
  ModelsIcon,
} from "@/components/icons";
import { Onboarding } from "@/components/onboarding/Onboarding";
import { useOnboarding } from "@/hooks/useOnboarding";
import type { AppSettings } from "@/bindings";
import "./App.css";

type SectionId = "general" | "models" | "advanced" | "diagnostics";

interface SectionDef {
  id: SectionId;
  label: string;
  icon: React.ReactNode;
  render: (settings: AppSettings, active: boolean) => React.ReactNode;
}

const SECTIONS: SectionDef[] = [
  {
    id: "general",
    label: "General",
    icon: <GeneralIcon />,
    render: (s) => <GeneralSettings settings={s} />,
  },
  {
    id: "models",
    label: "Models",
    icon: <ModelsIcon />,
    render: (s) => <ModelsSettings settings={s} />,
  },
  {
    id: "advanced",
    label: "Advanced",
    icon: <AdvancedIcon />,
    render: (s) => <AdvancedSettings settings={s} />,
  },
  {
    id: "diagnostics",
    label: "Diagnostics",
    icon: <DiagnosticsIcon />,
    render: (_s, active) => <DiagnosticsSettings active={active} />,
  },
];

function App() {
  const { ready, needsOnboarding, recheck } = useOnboarding();

  // Avoid a flash of either UI until prerequisites resolve.
  if (!ready) {
    return (
      <div className="flex h-screen w-screen items-center justify-center">
        <p className="text-secondary text-[13px]">Loading…</p>
      </div>
    );
  }

  if (needsOnboarding) {
    return <Onboarding onComplete={() => void recheck()} />;
  }

  return <SettingsApp />;
}

function SettingsApp() {
  const { settings, loading, initialize } = useSettingsStore();
  const [active, setActive] = useState<SectionId>("general");

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const current = SECTIONS.find((s) => s.id === active) ?? SECTIONS[0];

  return (
    <div className="flex h-screen w-screen overflow-hidden text-label">
      {/* Sidebar (transparent — lets window vibrancy show through) */}
      <aside
        className="flex w-[215px] shrink-0 flex-col px-2.5"
        style={{ borderRight: "0.5px solid var(--separator)" }}
      >
        {/* Top inset clears the traffic-light / titlebar region. */}
        <nav className="flex flex-col gap-0.5 pt-9">
          {SECTIONS.map((section) => (
            <SidebarItem
              key={section.id}
              label={section.label}
              icon={section.icon}
              active={active === section.id}
              onClick={() => setActive(section.id)}
            />
          ))}
        </nav>
      </aside>

      {/* Content */}
      <main className="mac-scroll flex-1 overflow-y-auto">
        <div className="mx-auto max-w-2xl px-5 pb-8 pt-9">
          {loading && !settings && (
            <p className="text-secondary text-[13px]">Loading settings…</p>
          )}
          {settings && current.render(settings, active === current.id)}
        </div>
      </main>
    </div>
  );
}

export default App;

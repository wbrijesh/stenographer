import { useEffect, useState } from "react";
import { useSettingsStore } from "@/stores/settingsStore";
import { SidebarItem } from "@/components/ui";
import {
  AdvancedSettings,
  GeneralSettings,
  ModelsSettings,
} from "@/components/settings";
import {
  AdvancedIcon,
  GeneralIcon,
  ModelsIcon,
} from "@/components/icons";
import type { AppSettings } from "@/bindings";
import "./App.css";

type SectionId = "general" | "models" | "advanced";

interface SectionDef {
  id: SectionId;
  label: string;
  icon: React.ReactNode;
  render: (settings: AppSettings) => React.ReactNode;
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
];

function App() {
  const { settings, loading, initialize } = useSettingsStore();
  const [active, setActive] = useState<SectionId>("general");

  useEffect(() => {
    void initialize();
  }, [initialize]);

  const current = SECTIONS.find((s) => s.id === active) ?? SECTIONS[0];

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background text-text">
      {/* Sidebar */}
      <aside className="flex w-48 shrink-0 flex-col border-r border-black/10 bg-black/[0.02] px-2.5 pt-4">
        <div className="px-2 pb-4">
          <h1 className="text-base font-semibold tracking-tight">
            Stenographer
          </h1>
          <p className="text-[11px] text-black/40">Settings</p>
        </div>
        <nav className="flex flex-col gap-1">
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
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-2xl px-6 py-7">
          {loading && !settings && (
            <p className="text-sm text-black/50">Loading settings…</p>
          )}
          {settings && current.render(settings)}
        </div>
      </main>
    </div>
  );
}

export default App;

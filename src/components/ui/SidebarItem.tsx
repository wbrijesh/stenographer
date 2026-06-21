import type { ReactNode } from "react";

interface SidebarItemProps {
  label: string;
  icon: ReactNode;
  active: boolean;
  onClick: () => void;
}

export function SidebarItem({ label, icon, active, onClick }: SidebarItemProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={
        active
          ? { background: "var(--accent)", color: "var(--accent-fg)" }
          : undefined
      }
      className={`flex h-7 w-full items-center gap-2.5 rounded-md px-2 text-[13px] transition-colors ${
        active
          ? "font-medium shadow-sm"
          : "text-label hover:bg-[var(--sidebar-hover)]"
      }`}
    >
      <span className="flex h-4 w-4 shrink-0 items-center justify-center">
        {icon}
      </span>
      <span className="truncate">{label}</span>
    </button>
  );
}

import type { ReactNode } from "react";

type ChipVariant = "model" | "skills" | "mcps" | "schedule" | "tools" | "stdio" | "http" | "sse" | "plugin" | "worker" | "enabled" | "disabled" | "running" | "stopped" | "starting" | "error";

interface MetaChipProps {
  variant: ChipVariant;
  icon?: ReactNode;
  children: ReactNode;
}

export function MetaChip({ variant, icon, children }: MetaChipProps) {
  return (
    <span className={`meta-chip meta-chip--${variant}`}>
      {icon && <span className="meta-chip__icon">{icon}</span>}
      {children}
    </span>
  );
}

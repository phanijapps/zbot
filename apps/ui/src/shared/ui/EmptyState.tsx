// ============================================================================
// EMPTY STATE COMPONENT
// Reusable component for displaying empty states with optional action
// ============================================================================

import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface EmptyStateProps {
  /** Icon to display (from lucide-react) */
  icon?: LucideIcon;
  /** Title text */
  title: string;
  /** Optional description */
  description?: string;
  /** Optional action button */
  action?: {
    label: string;
    onClick: () => void;
  };
  /** Optional hint displayed below the action */
  hint?: ReactNode;
  /** Size variant */
  size?: "sm" | "md" | "lg";
}

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  hint,
  size = "md",
}: EmptyStateProps) {
  const sizeConfig = {
    sm: {
      iconContainer: 48,
      icon: 20,
      padding: "var(--spacing-6)",
    },
    md: {
      iconContainer: 64,
      icon: 28,
      padding: "var(--spacing-12)",
    },
    lg: {
      iconContainer: 80,
      icon: 36,
      padding: "var(--spacing-16)",
    },
  };

  const config = sizeConfig[size];

  return (
    <div className="empty-state" style={{ padding: `${config.padding} var(--spacing-6)` }}>
      {Icon && (
        <div className="empty-state__icon" style={{ width: config.iconContainer, height: config.iconContainer }}>
          <Icon style={{ width: config.icon, height: config.icon }} />
        </div>
      )}
      <h3 className="empty-state__title">{title}</h3>
      {description && <p className="empty-state__description">{description}</p>}
      {action && (
        <button className="btn btn--primary btn--md" onClick={action.onClick}>
          {action.label}
        </button>
      )}
      {hint && <div className="empty-state__hint">{hint}</div>}
    </div>
  );
}

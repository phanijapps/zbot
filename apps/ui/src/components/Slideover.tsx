import { useId, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { useDialogFocus } from "@/hooks/useDialogFocus";

interface SlideoverProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  subtitle?: ReactNode;
  icon?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
}

export function Slideover({ open, onClose, title, subtitle, icon, children, footer, className }: SlideoverProps) {
  const panelRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useDialogFocus(open, panelRef, onClose);

  if (!open) return null;

  return (
    <>
      <div className="slideover-backdrop slideover-backdrop--open" onClick={onClose} aria-hidden="true" />
      <div ref={panelRef} tabIndex={-1} className={`slideover slideover--open ${className || ""}`} role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="slideover__header">
          <div className="slideover__header-left">
            {icon && <div className="slideover__icon">{icon}</div>}
            <div>
              <h2 id={titleId} className="slideover__title">{title}</h2>
              {subtitle && <div className="slideover__subtitle">{subtitle}</div>}
            </div>
          </div>
          <button className="slideover__close" onClick={onClose} aria-label="Close">
            <X style={{ width: 18, height: 18 }} />
          </button>
        </div>
        <div className="slideover__body">{children}</div>
        {footer && <div className="slideover__footer">{footer}</div>}
      </div>
    </>
  );
}

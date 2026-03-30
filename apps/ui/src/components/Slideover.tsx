import { useEffect, useCallback, type ReactNode } from "react";
import { X } from "lucide-react";

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
  const handleEscape = useCallback((e: KeyboardEvent) => {
    if (e.key === "Escape") onClose();
  }, [onClose]);

  useEffect(() => {
    if (open) {
      document.addEventListener("keydown", handleEscape);
      document.body.style.overflow = "hidden";
    }
    return () => {
      document.removeEventListener("keydown", handleEscape);
      document.body.style.overflow = "";
    };
  }, [open, handleEscape]);

  if (!open) return null;

  return (
    <>
      <div className="slideover-backdrop slideover-backdrop--open" onClick={onClose} aria-hidden="true" />
      <div className={`slideover slideover--open ${className || ""}`} role="dialog" aria-modal="true">
        <div className="slideover__header">
          <div className="slideover__header-left">
            {icon && <div className="slideover__icon">{icon}</div>}
            <div>
              <h2 className="slideover__title">{title}</h2>
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

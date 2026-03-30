import type { ReactNode } from "react";
import { HelpCircle } from "lucide-react";

interface HelpBoxProps {
  children: ReactNode;
  icon?: ReactNode;
}

export function HelpBox({ children, icon }: HelpBoxProps) {
  return (
    <div className="help-box">
      <div className="help-box__icon">
        {icon || <HelpCircle style={{ width: 16, height: 16 }} />}
      </div>
      <div className="help-box__content">{children}</div>
    </div>
  );
}

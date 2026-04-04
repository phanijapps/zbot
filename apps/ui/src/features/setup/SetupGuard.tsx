import { useEffect, useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { Loader2 } from "lucide-react";
import { getTransport } from "@/services/transport";

interface SetupGuardProps {
  children: React.ReactNode;
}

export function SetupGuard({ children }: SetupGuardProps) {
  const [isChecking, setIsChecking] = useState(true);
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    if (location.pathname === "/setup") {
      setIsChecking(false);
      return;
    }

    const cached = sessionStorage.getItem("setupComplete");
    if (cached === "true") {
      setIsChecking(false);
      return;
    }

    const check = async () => {
      try {
        const transport = await getTransport();
        const result = await transport.getSetupStatus();
        if (result.success && result.data) {
          if (result.data.setupComplete || result.data.hasProviders) {
            sessionStorage.setItem("setupComplete", "true");
          } else {
            navigate("/setup", { replace: true });
            return;
          }
        }
      } catch {
        // If check fails, don't block
      }
      setIsChecking(false);
    };
    check();
  }, [navigate, location.pathname]);

  if (isChecking) {
    return (
      <div className="loading-spinner">
        <Loader2 className="loading-spinner__icon" />
      </div>
    );
  }

  return <>{children}</>;
}

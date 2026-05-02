import { useEffect, useState } from "react";

type MdnsStatus = {
  active: boolean;
  interfaces: string[];
  aliasClaimed: boolean;
  instanceId: string;
};

type NetworkInfo = {
  exposeToLan: boolean;
  bindHost: string;
  port: number;
  hostnameUrls: string[];
  ipUrls: string[];
  mdns: MdnsStatus;
};

type ApiEnvelope<T> = { success: boolean; data?: T; error?: string };

export function NetworkSettingsCard() {
  const [info, setInfo] = useState<NetworkInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function load() {
      try {
        const res = await fetch("/api/network/info");
        const body = (await res.json()) as ApiEnvelope<NetworkInfo>;
        if (!cancelled) {
          if (body.success && body.data) {
            setInfo(body.data);
          } else {
            setError(body.error ?? "Failed to load network info");
          }
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) return <div className="settings-card">Loading network status…</div>;
  if (error) return <div className="settings-card error">{error}</div>;
  if (!info) return null;

  return (
    <section className="settings-card network-settings-card">
      <header>
        <h3>Network</h3>
      </header>

      {!info.exposeToLan && (
        <p className="muted">
          LAN exposure is off. Turn it on to make this daemon reachable from other devices.
        </p>
      )}
    </section>
  );
}

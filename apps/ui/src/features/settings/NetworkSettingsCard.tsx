import { useEffect, useState } from "react";
import { QRCodeSVG } from "qrcode.react";

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

function primaryUrl(info: NetworkInfo): string | null {
  return info.hostnameUrls[0] ?? info.ipUrls[0] ?? null;
}

export function NetworkSettingsCard() {
  const [info, setInfo] = useState<NetworkInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showRestartBanner, setShowRestartBanner] = useState(false);

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

  async function onToggle() {
    if (!info) return;
    const next = !info.exposeToLan;
    // Optimistic UI flip.
    setInfo({ ...info, exposeToLan: next });

    // Fetch current settings to preserve nested fields the user might have changed.
    const getRes = await fetch("/api/settings/network");
    const getBody = (await getRes.json()) as ApiEnvelope<{
      exposeToLan: boolean;
      discovery: Record<string, unknown>;
      advanced: { bindHost: string | null; httpPort: number };
    }>;
    const current = getBody.data ?? {
      exposeToLan: info.exposeToLan,
      discovery: {},
      advanced: { bindHost: null, httpPort: info.port },
    };

    const putRes = await fetch("/api/settings/network", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ...current, exposeToLan: next }),
    });
    const putBody = (await putRes.json()) as ApiEnvelope<unknown>;
    if (putBody.success) {
      setShowRestartBanner(true);
    } else {
      // Revert optimistic flip on failure.
      setInfo({ ...info, exposeToLan: info.exposeToLan });
      setError(putBody.error ?? "Failed to update network settings");
    }
  }

  if (loading) return <div className="settings-card">Loading network status…</div>;
  if (error) return <div className="settings-card error">{error}</div>;
  if (!info) return null;

  const qrTarget = primaryUrl(info);

  return (
    <section className="settings-card network-settings-card">
      <header className="row">
        <h3>Network</h3>
        <label className="toggle">
          <input
            type="checkbox"
            aria-label="Expose to LAN"
            checked={info.exposeToLan}
            onChange={() => void onToggle()}
          />
          <span>Expose to LAN</span>
        </label>
      </header>
      <p className="muted small">
        Other devices on your network can reach this daemon. Restart daemon to apply changes.
      </p>
      {showRestartBanner && (
        <div className="banner restart-required" role="status">
          Daemon restart required to apply changes.
        </div>
      )}

      {!info.exposeToLan && (
        <p className="muted">
          LAN exposure is off. Turn it on to make this daemon reachable from other devices.
        </p>
      )}

      {info.exposeToLan && (
        <>
          {!info.mdns.active && (
            <div className="warning" role="status">
              ⚠ mDNS responder failed to start — devices can still reach the IP URL above.
            </div>
          )}

          {info.mdns.active && !info.mdns.aliasClaimed && (
            <div className="info" role="status">
              <code>zbot.local</code> is already in use on this network — only the
              per-instance hostname is being advertised.
            </div>
          )}

          <div className="network-urls">
            <div className="url-list">
              <strong>Reachable at:</strong>
              <ul>
                {info.hostnameUrls.map((u) => (
                  <li key={u}>
                    <code>{u}</code>
                  </li>
                ))}
                {info.ipUrls.map((u) => (
                  <li key={u}>
                    <code>{u}</code>
                  </li>
                ))}
              </ul>
            </div>
            {qrTarget && (
              <div className="qr" data-testid="network-qr">
                <QRCodeSVG value={qrTarget} size={128} includeMargin />
              </div>
            )}
          </div>

          {info.mdns.interfaces.length > 0 && (
            <div className="status muted">
              ● Advertising on {info.mdns.interfaces.join(", ")}
            </div>
          )}
        </>
      )}
    </section>
  );
}

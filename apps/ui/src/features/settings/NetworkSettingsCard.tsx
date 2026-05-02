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

  const qrTarget = primaryUrl(info);

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

      {info.exposeToLan && (
        <>
          {!info.mdns.active && (
            <div className="warning" role="status">
              ⚠ mDNS responder failed to start — devices can still reach the IP URL above.
            </div>
          )}

          {info.mdns.active && !info.mdns.aliasClaimed && (
            <div className="info" role="status">
              <code>agentzero.local</code> is already in use on this network — only the
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

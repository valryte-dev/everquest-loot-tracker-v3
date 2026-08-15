import { useEffect, useMemo, useState } from "react";
import { FEATURES, type FeatureKey } from "./features";
import { bootstrapStatus } from "../shared/backend";
import type { BootstrapStatus, LoadingState } from "../shared/contracts";

const featureFromHash = (): FeatureKey => {
  const value = window.location.hash.replace(/^#\/?/, "") as FeatureKey;
  return FEATURES.some((feature) => feature.key === value) ? value : "live";
};

export function App() {
  const [active, setActive] = useState<FeatureKey>(featureFromHash);
  const [status, setStatus] = useState<LoadingState<BootstrapStatus>>({ kind: "loading" });
  const feature = useMemo(() => FEATURES.find((item) => item.key === active)!, [active]);

  useEffect(() => {
    const onHash = () => setActive(featureFromHash());
    window.addEventListener("hashchange", onHash);
    bootstrapStatus().then(
      (value) => setStatus({ kind: "ready", value }),
      (error: unknown) => setStatus({ kind: "error", message: error instanceof Error ? error.message : String(error) }),
    );
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  return (
    <div className="app-shell">
      <aside className="rail">
        <div className="brand"><span className="brand-mark">EQ</span><span><strong>Loot Tracker</strong><small>Version 3</small></span></div>
        <nav aria-label="Primary">
          {FEATURES.map((item) => (
            <a key={item.key} href={`#/${item.key}`} className={item.key === active ? "active" : ""}>
              <span className="nav-icon" aria-hidden="true">{item.icon}</span><span>{item.shortLabel}</span>
            </a>
          ))}
        </nav>
      </aside>
      <header className="global-bar">
        <div><span className="eyebrow">EverQuest workspace</span><strong>{feature.label}</strong></div>
        <div className={`health ${status.kind}`}>
          <span className="health-dot" />
          {status.kind === "loading" && "Starting V3…"}
          {status.kind === "error" && "Startup needs attention"}
          {status.kind === "ready" && (status.value.databaseReady ? "Local database ready" : "UI preview mode")}
        </div>
      </header>
      <main className="workspace">
        <section className="page-heading">
          <div><span className="eyebrow">Porting phase {feature.phase}</span><h1>{feature.label}</h1><p>{feature.description}</p></div>
          <button type="button" className="primary-action">Primary action</button>
        </section>
        <section className="summary-grid" aria-label="Workspace status">
          <article><span>V3 state</span><strong>{status.kind === "ready" ? "Initialized" : status.kind}</strong><small>Shared typed command contract</small></article>
          <article><span>Database</span><strong>{status.kind === "ready" ? `Schema ${status.value.schemaVersion}` : "—"}</strong><small>{status.kind === "ready" ? status.value.databasePath : "Resolving application data path"}</small></article>
          <article><span>Platform</span><strong>{status.kind === "ready" ? status.value.platform : "—"}</strong><small>Windows · macOS · Linux</small></article>
        </section>
        <section className="content-card">
          <div className="card-toolbar"><div className="filter"><input aria-label={`Filter ${feature.label}`} placeholder={`Filter ${feature.shortLabel.toLowerCase()}…`} /><button type="button" aria-label="Clear filter">×</button></div><button type="button">Sort</button></div>
          <div className="empty-state"><span className="empty-icon">{feature.icon}</span><h2>{feature.label} V3 workspace</h2><p>The cross-platform foundation is active. Feature data appears here as each V2 workflow is moved behind the new typed Rust command boundary.</p></div>
        </section>
      </main>
    </div>
  );
}

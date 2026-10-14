import { useState, type CSSProperties } from "react";
import { useReveal } from "./reveal";
import {
  assetUrl,
  confirm,
  EVENTS_PROTOCOL,
  getPluginApi,
  hostVersion,
  notify,
  usePluginI18n,
  usePomContext,
  usePomEvent,
  type NotificationResponse,
  type PomEvent,
} from "../host/runtime";
import { useScreenReady } from "../host/ready";

const STEPS = ["one", "two", "three", "four", "five"] as const;
const FEATURES = [
  { key: "menu", icon: "01" },
  { key: "screens", icon: "02" },
  { key: "locales", icon: "03" },
  { key: "workspace", icon: "04" },
  { key: "preferences", icon: "05" },
  { key: "events", icon: "06" },
  { key: "notifications", icon: "07" },
  { key: "store", icon: "08" },
] as const;
const MAX_EVENTS = 12;

/** Stagger index for the reveal animations (`--i` in plugin.css). */
const order = (index: number) => ({ "--i": index }) as CSSProperties;

const SNIPPET = `import { confirm, notify, usePomEvent } from "./host/runtime";

// POM -> plugin: the same envelope on every channel
usePomEvent("theme.changed", (event) => {
  applyTheme(event.payload.theme); // "light" | "dark"
});

// plugin -> POM: ask this user and read the answer here
const answer = await confirm({ title: "Deploy the model?" });
if (answer.action === "accepted") {
  await notify({ title: "Deploy started", scope: "network" });
}`;

type BackendEvents = { status: "idle" | "loading" | "ready" | "unavailable"; events: PomEvent[] };

function eventSummary(event: PomEvent): string {
  const payload = event.payload as Record<string, unknown>;
  if (typeof payload.locale === "string") return payload.locale;
  if (typeof payload.theme === "string") return payload.theme;
  if (typeof payload.action === "string") return `${payload.kind ?? ""} ${payload.action}`.trim();
  if (typeof payload.title === "string") return payload.title;
  if (typeof payload.model === "string") return payload.model;
  return event.target;
}

export function Tutorial() {
  const { t } = usePluginI18n();
  const context = usePomContext();
  const [events, setEvents] = useState<PomEvent[]>([]);
  const [lastResponse, setLastResponse] = useState<NotificationResponse | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [backend, setBackend] = useState<BackendEvents>({ status: "idle", events: [] });
  const revealRef = useReveal<HTMLElement>();
  // Everything this screen draws on mount is static: the events list starts in
  // its real empty state and the backend answer only loads on click, so the
  // screen is ready as soon as it mounts.
  useScreenReady(true);

  usePomEvent("*", (event) => {
    setEvents((current) => [event, ...current.filter((item) => item.id !== event.id)].slice(0, MAX_EVENTS));
  }, true);

  async function run(action: string, command: () => Promise<NotificationResponse>) {
    setBusy(action);
    try {
      setLastResponse(await command());
    } finally {
      setBusy(null);
    }
  }

  async function loadBackendEvents() {
    setBackend((current) => ({ ...current, status: "loading" }));
    try {
      const value = await getPluginApi<{ protocol?: string; events?: PomEvent[] }>("events");
      const received = value.protocol === EVENTS_PROTOCOL && Array.isArray(value.events) ? value.events : [];
      setBackend({ status: "ready", events: received });
    } catch {
      setBackend({ status: "unavailable", events: [] });
    }
  }

  const responseLabel = lastResponse ? t(`demo.response.${lastResponse.action}`) : null;

  return (
    <main className="pb-page" ref={revealRef}>
      <section className="pb-hero pb-hero-split pb-hero-lit">
        <img className="pb-hero-mark" src={assetUrl("ui/pom-mark-dark.png")} alt="" aria-hidden="true" />
        <p className="pb-eyebrow pb-eyebrow-hero"><span className="pb-tick" aria-hidden="true" />{t("hero.eyebrow")}</p>
        <h1 className="pb-hero-title">
          <span>{t("hero.titleA")}</span>
          <span className="pb-gradient-text">{t("hero.titleB")}</span>
        </h1>
        <p className="pb-hero-sub">{t("hero.body")}</p>
        <ul className="pb-chips" aria-label={t("hero.contextLabel")}>
          <li><span>{t("hero.sdk")}</span><strong>v{hostVersion()}</strong></li>
          <li><span>{t("hero.locale")}</span><strong>{context.locale}</strong></li>
          <li><span>{t("hero.theme")}</span><strong>{t(`theme.${context.theme}`)}</strong></li>
          <li><span>{t("hero.protocol")}</span><strong>{EVENTS_PROTOCOL}</strong></li>
        </ul>
      </section>

      <section className="pb-section" aria-labelledby="pb-steps-title">
        <p className="pb-eyebrow pb-reveal"><span className="pb-tick" aria-hidden="true" />01 / {t("steps.label")}</p>
        <h2 id="pb-steps-title" className="pb-h2 pb-reveal">{t("steps.title")}</h2>
        <ol className="pb-steps pb-reveal">
          {STEPS.map((step, index) => (
            <li key={step} className="pb-step pb-reveal" style={order(index)}>
              <span className="pb-step-num" aria-hidden="true">{index + 1}</span>
              <div>
                <h3 className="pb-h3">{t(`steps.${step}.title`)}</h3>
                <p>{t(`steps.${step}.body`)}</p>
                <code className="pb-path">{t(`steps.${step}.file`)}</code>
              </div>
            </li>
          ))}
        </ol>
      </section>

      <section className="pb-section" aria-labelledby="pb-features-title">
        <p className="pb-eyebrow pb-reveal"><span className="pb-tick" aria-hidden="true" />02 / {t("features.label")}</p>
        <h2 id="pb-features-title" className="pb-h2 pb-reveal">{t("features.title")}</h2>
        <div className="pb-grid">
          {FEATURES.map((feature, index) => (
            <article key={feature.key} className="pb-card pb-card-feature pb-reveal" style={order(index % 4)}>
              <p className="pb-card-num">{feature.icon}</p>
              <h3 className="pb-h3">{t(`features.${feature.key}.title`)}</h3>
              <p>{t(`features.${feature.key}.body`)}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="pb-section" aria-labelledby="pb-demo-title">
        <p className="pb-eyebrow pb-reveal"><span className="pb-tick" aria-hidden="true" />03 / {t("demo.label")}</p>
        <h2 id="pb-demo-title" className="pb-h2 pb-reveal">{t("demo.title")}</h2>
        <p className="pb-lede pb-reveal">{t("demo.body")}</p>
        <div className="pb-demo">
          <article className="pb-card pb-card-accent pb-reveal" style={order(0)}>
            <h3 className="pb-h3">{t("demo.actionsTitle")}</h3>
            <div className="pb-btn-row">
              <button
                type="button"
                className="pb-btn pb-btn-outline"
                disabled={busy !== null}
                onClick={() => void run("local", () => notify({ title: t("demo.localTitle"), body: t("demo.localBody"), level: "success" }))}
              >
                {t("demo.notifyLocal")}
              </button>
              <button
                type="button"
                className="pb-btn pb-btn-outline"
                disabled={busy !== null}
                onClick={() => void run("network", () => notify({ title: t("demo.networkTitle"), body: t("demo.networkBody"), level: "info", scope: "network" }))}
              >
                {t("demo.notifyNetwork")}
              </button>
              <button
                type="button"
                className="pb-btn pb-btn-accent"
                disabled={busy !== null}
                onClick={() => void run("confirm", () => confirm({
                  title: t("demo.confirmTitle"),
                  body: t("demo.confirmBody"),
                  acceptLabel: t("demo.confirmAccept"),
                  cancelLabel: t("demo.confirmCancel"),
                }))}
              >
                {t("demo.askConfirm")}
              </button>
              <button
                type="button"
                className="pb-btn pb-btn-outline"
                disabled={busy !== null}
                onClick={() => void run("dialog", () => confirm({
                  title: t("demo.confirmTitle"),
                  body: t("demo.confirmBody"),
                  acceptLabel: t("demo.confirmAccept"),
                  cancelLabel: t("demo.confirmCancel"),
                  display: "dialog",
                }))}
              >
                {t("demo.askDialog")}
              </button>
            </div>
            <div className="pb-response" role="status" aria-live="polite">
              <span className="pb-response-label">{t("demo.responseLabel")}</span>
              {lastResponse ? (
                <>
                  <strong key={lastResponse.request_id} className={`pb-response-value pb-response-${lastResponse.action}`}>{responseLabel}</strong>
                  <code className="pb-path">{lastResponse.request_id.slice(0, 12)}</code>
                  {lastResponse.error && <span className="pb-muted">{lastResponse.error}</span>}
                </>
              ) : (
                <span className="pb-muted">{t("demo.noResponse")}</span>
              )}
            </div>
          </article>

          <article className="pb-card pb-reveal" style={order(1)}>
            <h3 className="pb-h3">{t("demo.eventsTitle")}</h3>
            <p className="pb-muted">{t("demo.eventsHint")}</p>
            {events.length === 0 ? (
              <p className="pb-empty">{t("demo.eventsEmpty")}</p>
            ) : (
              <ol className="pb-event-list">
                {events.map((event) => (
                  <li key={event.id}>
                    <code className="pb-event-type">{event.type}</code>
                    <span className="pb-event-summary">{eventSummary(event)}</span>
                    <time className="pb-muted" dateTime={event.at}>{new Date(event.at).toLocaleTimeString(context.locale)}</time>
                  </li>
                ))}
              </ol>
            )}
          </article>

          <article className="pb-card pb-reveal" style={order(2)}>
            <div className="pb-card-head">
              <h3 className="pb-h3">{t("demo.backendTitle")}</h3>
              <button type="button" className="pb-btn pb-btn-outline pb-btn-small" onClick={() => void loadBackendEvents()}>
                {t("demo.backendRefresh")}
              </button>
            </div>
            <p className="pb-muted">{t("demo.backendHint")}</p>
            {backend.status === "unavailable" && <p className="pb-empty">{t("demo.backendUnavailable")}</p>}
            {backend.status === "ready" && backend.events.length === 0 && <p className="pb-empty">{t("demo.backendEmpty")}</p>}
            {backend.events.length > 0 && (
              <ol className="pb-event-list">
                {backend.events.map((event) => (
                  <li key={event.id}>
                    <code className="pb-event-type">{event.type}</code>
                    <span className="pb-event-summary">{event.target}</span>
                    <time className="pb-muted" dateTime={event.at}>{new Date(event.at).toLocaleTimeString(context.locale)}</time>
                  </li>
                ))}
              </ol>
            )}
          </article>
        </div>
      </section>

      <section className="pb-section" aria-labelledby="pb-code-title">
        <p className="pb-eyebrow pb-reveal"><span className="pb-tick" aria-hidden="true" />04 / {t("code.label")}</p>
        <h2 id="pb-code-title" className="pb-h2 pb-reveal">{t("code.title")}</h2>
        <div className="pb-code-card pb-reveal">
          <p className="pb-code-label">ui/src/screens/Tutorial.tsx</p>
          <pre className="pb-code"><code>{SNIPPET}</code></pre>
        </div>
      </section>
    </main>
  );
}

import { useCallback, useEffect, useState } from "react";
import { notify, rpc, ui, usePluginI18n, usePomEvent } from "../host/runtime";
import { useScreenReady } from "../host/ready";

type Status = {
  plugin_code: string;
  version: string;
  context: { gateway: boolean; host_api: boolean; workspace_root: string | null };
  data_directory: string | null;
  ingested: number;
  events: { id: string; type: string; at: string }[];
};

type Refusal = { error: string; code?: string };

export function Main() {
  const { t } = usePluginI18n();
  const { PageHeader, Panel, PanelHeader, Button, Badge, KeyValue, Empty, ErrorState } = ui();
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState<unknown>(null);

  const load = useCallback(() => {
    rpc<Status>("status").then(
      (reply) => {
        setStatus(reply);
        setError(null);
      },
      setError,
    );
  }, []);

  useEffect(load, [load]);
  // Every event the node pushes reaches the backend too; reload to show it.
  usePomEvent("*", load);
  // Open the screen once the first answer settled, not on the first render.
  useScreenReady(status !== null || error !== null);

  const clear = async () => {
    const reply = await rpc<Refusal | { status: string }>("events.clear");
    // A refusal (for example a non-admin profile) is a readable body, not a failure.
    if ("error" in reply) void notify({ title: reply.error, level: "warning" });
    else load();
  };

  if (error && !status) {
    return <ErrorState title={t("main.title")} error={error} onRetry={load} />;
  }

  return (
    <div className="pb-page">
      <PageHeader
        title={t("main.title")}
        subtitle={t("main.subtitle")}
        actions={
          <Button variant="outline" size="sm" onClick={() => void notify({ title: t("main.notice") })}>
            {t("main.notify")}
          </Button>
        }
      />
      {status === null ? (
        <p className="pb-muted">{t("main.loading")}</p>
      ) : (
        <div className="pb-grid">
          <Panel>
            <PanelHeader title={`${status.plugin_code} ${status.version}`} />
            <dl className="pb-facts">
              <KeyValue label={t("main.gateway")}>
                <Badge tone={status.context.gateway ? "aqua" : "amber"}>
                  {status.context.gateway ? t("main.yes") : t("main.no")}
                </Badge>
              </KeyValue>
              <KeyValue label={t("main.workspace")}>{status.context.workspace_root ?? t("main.none")}</KeyValue>
              <KeyValue label={t("main.dataDirectory")}>{status.data_directory ?? t("main.none")}</KeyValue>
              <KeyValue label={t("main.ingested")}>{status.ingested}</KeyValue>
            </dl>
          </Panel>
          <Panel>
            <PanelHeader
              title={t("main.events")}
              actions={
                <Button variant="ghost" size="sm" onClick={() => void clear()}>
                  {t("main.clear")}
                </Button>
              }
            />
            {status.events.length === 0 ? (
              <Empty title={t("main.noEvents")} />
            ) : (
              <ul className="pb-events">
                {status.events.map((event) => (
                  <li key={event.id}>
                    <Badge mono>{event.type}</Badge> <span className="pb-muted">{event.at}</span>
                  </li>
                ))}
              </ul>
            )}
          </Panel>
        </div>
      )}
    </div>
  );
}

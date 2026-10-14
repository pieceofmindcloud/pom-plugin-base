import { useEffect, useState } from "react";
import { getPluginApi, usePluginI18n } from "../host/runtime";
import { useScreenReady } from "../host/ready";

type WorkspaceStatus = "loading" | "unavailable" | "empty" | "ready";

type WorkspaceState = {
  status: WorkspaceStatus;
  projects: string[];
};

const loadingState: WorkspaceState = { status: "loading", projects: [] };

export function normalizeWorkspace(value: unknown): WorkspaceState {
  if (typeof value !== "object" || value === null) {
    return { status: "unavailable", projects: [] };
  }

  const payload = value as { status?: unknown; projects?: unknown };
  const status = payload.status;
  const projects = payload.projects;
  if (
    (status !== "ready" && status !== "empty" && status !== "unavailable") ||
    !Array.isArray(projects) ||
    !projects.every((project): project is string => typeof project === "string")
  ) {
    return { status: "unavailable", projects: [] };
  }

  if (status === "unavailable") {
    return { status, projects: [] };
  }
  if (projects.length === 0) {
    return { status: "empty", projects: [] };
  }
  if (status === "empty") {
    return { status: "unavailable", projects: [] };
  }
  return { status: "ready", projects };
}

function useWorkspace(): WorkspaceState {
  const [state, setState] = useState(loadingState);

  useEffect(() => {
    let active = true;
    getPluginApi<unknown>("projects")
      .then((value) => {
        if (active) setState(normalizeWorkspace(value));
      })
      .catch(() => {
        if (active) setState({ status: "unavailable", projects: [] });
      });
    return () => {
      active = false;
    };
  }, []);

  return state;
}

export function Projects() {
  const { t } = usePluginI18n();
  const workspace = useWorkspace();
  const description = t("projects.body");
  // The workspace list is what this screen is for: the host opens it when the
  // answer arrived (or failed), not over the loading card.
  useScreenReady(workspace.status !== "loading");

  return (
    <main className="pb-page">
      <section className="pb-hero pb-hero-compact pb-hero-lit" aria-live="polite">
        <p className="pb-eyebrow pb-eyebrow-hero"><span className="pb-tick" aria-hidden="true" />{t("projects.eyebrow")}</p>
        <h1 className="pb-hero-title">
          <span>{t("projects.titleA")}</span>
          <span className="pb-gradient-text">{t("projects.titleB")}</span>
        </h1>
        <p className="pb-hero-sub">{description}</p>
      </section>

      <section className="pb-section">
        {workspace.status === "loading" && (
          <p className="pb-card pb-state" role="status">{t("projects.loading")}</p>
        )}
        {workspace.status === "unavailable" && (
          <p className="pb-card pb-state pb-muted" role="status">{t("projects.unavailable")}</p>
        )}
        {workspace.status === "empty" && (
          <p className="pb-card pb-state pb-muted" role="status">{t("projects.empty")}</p>
        )}
        {workspace.status === "ready" && (
          <>
            <p className="pb-eyebrow"><span className="pb-tick" aria-hidden="true" />{t("projects.count", { count: workspace.projects.length })}</p>
            <ul className="pb-project-list" aria-label={description}>
              {workspace.projects.map((project, index) => (
                <li key={project} className="pb-card pb-project" style={{ animationDelay: `${Math.min(index, 12) * 45}ms` }}>
                  <span className="pb-project-mark" aria-hidden="true">{project.slice(0, 1).toUpperCase()}</span>
                  <span className="pb-project-name">{project}</span>
                </li>
              ))}
            </ul>
          </>
        )}
      </section>
    </main>
  );
}

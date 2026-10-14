/**
 * The "finished rendering" signal the POM waits for (host version 6).
 *
 * The host imports this bundle and mounts it, but mounting is not having the
 * screen on screen: the first workspace answer is still on its way, and showing
 * the screen that early is a list that fills in front of the person. So the POM
 * keeps its pieces animation covering the screen - mounted, but out of sight -
 * until we tell it we are done, or until its short wait ends.
 *
 * `useScreenReady` sends that signal once per mount, the first time `ready`
 * turns true: the first workspace answer settled, not the first render. A POM
 * older than version 6 has no `ready` on the host object: the call is simply
 * skipped and the screen opens on its own.
 */
import { useEffect, useRef } from "react";

type ReadyHost = { ready?: unknown };

export function useScreenReady(ready: boolean) {
  const sent = useRef(false);
  useEffect(() => {
    if (sent.current || !ready) return;
    const host = (globalThis as { __POM_HOST__?: ReadyHost }).__POM_HOST__;
    if (typeof host?.ready !== "function") return;
    sent.current = true;
    (host.ready as () => void)();
  }, [ready]);
}
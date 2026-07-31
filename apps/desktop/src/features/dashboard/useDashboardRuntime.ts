import { useCallback, useEffect, useRef, useState } from "react";
import type { Event, UnlistenFn } from "@tauri-apps/api/event";
import { invoke, tauriEvent } from "../../lib/tauri";
import { parseTunnelArguments } from "../../lib/profile";
import type { ActivityLog, DashboardTab, ProfileEditor, RuntimeStatus, ToolActivityEvent, TunnelProvider } from "../../types";

/** Runtime dependencies provided by the dashboard editor and tunnel form. */
type RuntimeOptions = {
  editor: ProfileEditor;
  tunnelProvider: TunnelProvider;
  tunnelExecutable: string;
  tunnelArgs: string;
  discardToml: (action: string) => Promise<boolean>;
};

/** Coordinates server/tunnel processes and the live activity event stream. */
export function useDashboardRuntime({ editor, tunnelProvider, tunnelExecutable, tunnelArgs, discardToml }: RuntimeOptions) {
  const [running, setRunning] = useState(false);
  const [serverBusy, setServerBusy] = useState(false);
  const [serverError, setServerError] = useState("");
  const [runtimeProfileId, setRuntimeProfileId] = useState<string | null>(null);
  const [logs, setLogs] = useState<ActivityLog[]>([]);
  const [unreadActivity, setUnreadActivity] = useState(0);
  const [tunnelRunning, setTunnelRunning] = useState(false);
  const [activeTunnelProvider, setActiveTunnelProvider] = useState<TunnelProvider | null>(null);
  const [publicUrl, setPublicUrl] = useState<string | null>(null);
  const [tunnelBusy, setTunnelBusy] = useState(false);
  const activeTabRef = useRef<DashboardTab>("overview");
  const profileId = editor.profileState!.active_profile_id;

  useEffect(() => {
    const updateStatus = () => invoke<RuntimeStatus>("get_runtime_status").then((status) => {
      setRunning(status.running);
      setTunnelRunning(status.tunnel_running);
      setActiveTunnelProvider(status.tunnel_provider);
      setPublicUrl(status.public_url);
      if (status.tunnel_error) setServerError(status.tunnel_error);
    }).catch((error) => setServerError(String(error)));
    updateStatus();
    const timer = window.setInterval(updateStatus, 1000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: UnlistenFn | undefined;
    const addActivity = ({ payload }: Event<ToolActivityEvent>) => {
      const duration = payload.durationUs < 1000
        ? `${payload.durationUs} us`
        : `${(payload.durationUs / 1000).toFixed(payload.durationUs < 10000 ? 1 : 0)} ms`;
      const log: ActivityLog = {
        id: payload.id,
        time: new Date(payload.timestampMs).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }),
        tool: payload.tool,
        target: payload.target,
        duration,
        status: payload.status,
        client: payload.client,
        error: payload.error,
      };
      setLogs((current) => [log, ...current].slice(0, 200));
      if (activeTabRef.current !== "activity") setUnreadActivity((current) => current + 1);
    };
    tauriEvent.then((events) => events?.listen<ToolActivityEvent>("tool-activity", addActivity))
      .then((stopListening) => {
        if (!stopListening) return;
        if (disposed) stopListening();
        else unlisten = stopListening;
      })
      .catch((error) => console.error("Failed to listen for tool activity:", error));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  /** Starts the server after persisting staged profile edits. */
  const startServer = useCallback(async (): Promise<boolean> => {
    setServerBusy(true);
    setServerError("");
    try {
      if (!await discardToml("save the GUI profile and start the server")) return false;
      if (editor.dirty && !(await editor.saveProfile())) return false;
      await invoke("start_server", { profileId });
      setRunning(true);
      setRuntimeProfileId(profileId);
      return true;
    } catch (error) {
      setServerError(error instanceof Error ? error.message : String(error));
      return false;
    } finally {
      setServerBusy(false);
    }
  }, [editor.dirty, editor.saveProfile, profileId, discardToml]);

  /** Saves edits and replaces the currently running server instance. */
  const restartServer = useCallback(async () => {
    setServerBusy(true);
    setServerError("");
    try {
      if (!await discardToml("save the GUI profile and restart the server")) return;
      if (!(await editor.saveProfile())) return;
      if (tunnelRunning) await invoke("stop_tunnel");
      await invoke("stop_server");
      setRunning(false);
      setTunnelRunning(false);
      await invoke("start_server", { profileId });
      setRunning(true);
      setRuntimeProfileId(profileId);
    } catch (error) {
      setServerError(String(error));
    } finally {
      setServerBusy(false);
    }
  }, [editor.saveProfile, tunnelRunning, profileId, discardToml]);

  /** Stops the server and clears all related runtime state. */
  const stopServer = useCallback(async () => {
    setServerBusy(true);
    setServerError("");
    try {
      await invoke("stop_server");
      setRunning(false);
      setRuntimeProfileId(null);
      setTunnelRunning(false);
      setActiveTunnelProvider(null);
      setPublicUrl(null);
    } catch (error) {
      setServerError(error instanceof Error ? error.message : String(error));
    } finally {
      setServerBusy(false);
    }
  }, []);

  /** Starts the configured public tunnel, starting the server first when needed. */
  const connectTunnel = useCallback(async () => {
    setTunnelBusy(true);
    setServerError("");
    try {
      if (!editor.roots.length) {
        setServerError("Choose a workspace folder before starting remote access.");
        return;
      }
      if (!running && !(await startServer())) return;
      const connectedUrl = await invoke<string>("start_tunnel", {
        config: {
          provider: tunnelProvider,
          executable: tunnelExecutable,
          extra_args: parseTunnelArguments(tunnelArgs),
          host: editor.advanced.host,
          port: Number(editor.port),
        },
      });
      setTunnelRunning(true);
      setActiveTunnelProvider(tunnelProvider);
      setPublicUrl(connectedUrl);
    } catch (error) {
      setServerError(error instanceof Error ? error.message : String(error));
    } finally {
      setTunnelBusy(false);
    }
  }, [editor.roots, editor.advanced.host, editor.port, running, startServer, tunnelArgs, tunnelProvider, tunnelExecutable]);

  /** Stops the public tunnel without stopping the local server. */
  const disconnectTunnel = useCallback(async () => {
    setTunnelBusy(true);
    setServerError("");
    try {
      await invoke("stop_tunnel");
      setTunnelRunning(false);
      setActiveTunnelProvider(null);
      setPublicUrl(null);
    } catch (error) {
      setServerError(error instanceof Error ? error.message : String(error));
    } finally {
      setTunnelBusy(false);
    }
  }, []);

  /** Records dashboard navigation and resets activity notifications. */
  const visitTab = (tab: DashboardTab) => {
    activeTabRef.current = tab;
    if (tab === "activity") setUnreadActivity(0);
  };

  return {
    running, serverBusy, serverError, runtimeProfileId, logs, setLogs, unreadActivity,
    tunnelRunning, activeTunnelProvider, publicUrl, tunnelBusy, startServer, restartServer,
    stopServer, connectTunnel, disconnectTunnel, visitTab,
  };
}

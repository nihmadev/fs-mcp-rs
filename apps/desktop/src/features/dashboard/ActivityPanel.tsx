import { useState, type CSSProperties } from "react";
import type { ActivityLog } from "../../types";
import { Icon } from "../../components/ui/Icon";

/** Live activity page with animated log clearing. */
export function ActivityPanel({ logs, clearLogs }: { logs: ActivityLog[]; clearLogs: () => void }) {
  const [clearing, setClearing] = useState(false);
  const handleClear = () => {
    if (clearing) return;
    setClearing(true);
    window.setTimeout(() => {
      clearLogs();
      setClearing(false);
    }, 760);
  };
  return (
    <div className="dashboard-page narrow-page screen-enter">
      <div className="page-intro page-intro-actions"><div><h2>Activity</h2><p>Live tool calls handled by this desktop server.</p></div>{logs.length > 0 && <button className={`outlined-button clear-activity-button ${clearing ? "clearing" : ""}`} type="button" disabled={clearing} onClick={handleClear}><Icon name="delete_sweep" className="icon-trash" /> {clearing ? "Clearing..." : "Clear"}</button>}</div>
      <section className="dashboard-card activity-card">
        {logs.length > 0 ? logs.map((log, index) => <div className={clearing ? "log-row-removing" : ""} style={clearing ? { "--clear-index": index } as CSSProperties : undefined} key={log.id}><LogRow log={log} /></div>) : <div className="empty-state"><Icon name="receipt_long" /><strong>No tool calls yet</strong><p>Calls will appear here after the MCP server handles them.</p></div>}
      </section>
    </div>
  );
}

/** Detailed single activity record. */
function LogRow({ log }: { log: ActivityLog }) {
  return <div className="log-row"><span className={`log-status ${log.status}`}><Icon name={log.status === "ok" ? "check" : "error"} /></span><span className="log-main"><strong>{log.tool}</strong><small title={log.error ?? log.target}>{log.target}{log.error ? ` - ${log.error}` : ""}</small></span><span className="log-client" title={log.client}>{log.client}</span><span className="log-duration">{log.duration}</span><time>{log.time}</time></div>;
}

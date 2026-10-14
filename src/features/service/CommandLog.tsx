import { useEffect, useRef } from "react";
import type { CpaJob } from "./api";

export function CommandLog({ logs, feedback, busy }: { logs: CpaJob["logs"]; feedback: string; busy: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  const last = logs.at(-1)?.id;
  useEffect(() => {
    const element = ref.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [last, feedback]);
  return <section className="service-log">
    <div className="service-log-heading"><span>命令日志</span>{busy && <span className="muted">执行中…</span>}</div>
    <div ref={ref} className="service-log-output" role="log" aria-label="CPA 命令日志" tabIndex={0}>
      {logs.map(entry => <div className="service-log-line" key={entry.id}>
        <time>{new Date(entry.at * 1000).toLocaleTimeString("zh-CN", { hour12: false })}</time>
        <span>{entry.message}</span>
      </div>)}
      {feedback && feedback !== logs.at(-1)?.message && <div className="service-log-line">{feedback}</div>}
      {!logs.length && !feedback && <span className="muted">暂无命令</span>}
    </div>
  </section>;
}

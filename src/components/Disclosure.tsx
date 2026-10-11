import { useId, useState, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";
export function Disclosure({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <section className="disclosure" data-open={open}>
      <button
        type="button"
        className="disclosure-trigger"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen(!open)}
      >
        <span>{title}</span>
        <span>
          <ChevronDown size={12} />
        </span>
      </button>
      <div
        id={id}
        className="disclosure-panel"
        aria-hidden={!open}
        inert={!open}
      >
        <div>
          <div className="disclosure-content">{children}</div>
        </div>
      </div>
    </section>
  );
}

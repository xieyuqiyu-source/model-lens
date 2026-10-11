import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Check, ChevronDown } from "lucide-react";
import { Brand } from "./Common";
export type SelectOption = {
  value: string;
  label: string;
  brand?: string;
  detail?: string;
};
export function Select({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
}) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const selected = options.find((o) => o.value === value) ?? options[0];
  useEffect(() => {
    if (!open) return;
    panel.current
      ?.querySelector('[data-active="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [active, open]);
  useEffect(() => {
    if (!open) return;
    const close = () => panel.current?.hidePopover();
    window.addEventListener("resize", close);
    return () => window.removeEventListener("resize", close);
  }, [open]);
  function hide(restore = true) {
    panel.current?.hidePopover();
    if (restore) trigger.current?.focus();
  }
  function show() {
    const button = trigger.current,
      menu = panel.current;
    if (!button || !menu || !options.length) return;
    const rect = button.getBoundingClientRect();
    const width = Math.min(Math.max(rect.width, 220), window.innerWidth - 24);
    const height = Math.min(options.length * 42 + 16, 248);
    const below = window.innerHeight - rect.bottom - 12;
    const above = rect.top - 12;
    const up = below < height && above > below;
    menu.style.width = `${width}px`;
    menu.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`;
    menu.style.top = up ? "auto" : `${rect.bottom + 6}px`;
    menu.style.bottom = up ? `${window.innerHeight - rect.top + 6}px` : "auto";
    menu.style.maxHeight = `${Math.max(60, Math.min(248, (up ? above : below) - 6))}px`;
    menu.style.transformOrigin = up ? "bottom center" : "top center";
    setActive(
      Math.max(
        0,
        options.findIndex((o) => o.value === value),
      ),
    );
    menu.showPopover();
    menu.focus({ preventScroll: true });
  }
  function choose(index: number) {
    const option = options[index];
    if (option) onChange(option.value);
    hide();
  }
  function keyDown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      hide();
      return;
    }
    if (event.key === "Tab") {
      hide();
      return;
    }
    const arrows: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
    if (event.key in arrows) {
      event.preventDefault();
      setActive(
        (i) => (i + arrows[event.key] + options.length) % options.length,
      );
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      setActive(event.key === "Home" ? 0 : options.length - 1);
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      choose(active);
    } else if (event.key.length === 1) {
      const match = options.findIndex((o) =>
        o.label.toLowerCase().startsWith(event.key.toLowerCase()),
      );
      if (match >= 0) setActive(match);
    }
  }
  if (!("showPopover" in HTMLElement.prototype))
    return (
      <select
        className="select-trigger"
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>{option.label}</option>
        ))}
      </select>
    );
  return (
    <div className="select-control">
      <button
        ref={trigger}
        type="button"
        className="select-trigger"
        aria-label={`${label}：${selected?.label || "请选择"}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => (open ? hide() : show())}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown" || e.key === "ArrowUp") {
            e.preventDefault();
            show();
          }
        }}
      >
        {selected?.brand && <Brand name={selected.brand} />}
        <span>{selected?.label || "请选择"}</span>
        <ChevronDown size={13} />
      </button>
      <div
        ref={panel}
        id={id}
        popover="auto"
        data-open={open}
        role="listbox"
        tabIndex={-1}
        className="select-menu"
        aria-label={label}
        aria-activedescendant={`${id}-${active}`}
        onKeyDown={keyDown}
        onToggle={(e) => {
          const visible = e.newState === "open";
          setOpen(visible);
          if (visible)
            panel.current
              ?.querySelector(`[data-active="true"]`)
              ?.scrollIntoView({ block: "nearest" });
        }}
      >
        {options.map((option, index) => (
          <div
            role="option"
            id={`${id}-${index}`}
            key={option.value}
            aria-selected={option.value === value}
            data-active={active === index}
            className="select-option"
            onPointerMove={() => setActive(index)}
            onClick={() => choose(index)}
          >
            {option.brand && <Brand name={option.brand} />}
            <span>
              <strong>{option.label}</strong>
              {option.detail && <small>{option.detail}</small>}
            </span>
            {option.value === value && (
              <Check size={14} className="option-check" />
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

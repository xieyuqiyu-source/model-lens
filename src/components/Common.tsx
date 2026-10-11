import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { dismissSurface } from "./motion";
import { X, Circle } from "lucide-react";
export const Logo = () => (
  <span className="logo">
    <img src="/brands/modellens.svg" width="19" height="19" alt="" />
    <span>ModelLens</span>
  </span>
);
export function Brand({ name }: { name: string }) {
  if (name === "unknown")
    return <Circle className="brand" size={18} aria-label="其他厂商" />;
  return (
    <img
      className="brand"
      src={`/brands/${name}.svg`}
      alt={name}
      width="18"
      height="18"
    />
  );
}
export function Toggle({
  on,
  label,
  onChange,
  disabled = false,
}: {
  on: boolean;
  label: string;
  onChange: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className="switch"
      role="switch"
      disabled={disabled}
      aria-checked={on}
      aria-label={label}
      onClick={onChange}
    >
      <span />
    </button>
  );
}
export function Modal({
  title,
  children,
  onClose,
}: {
  title: string;
  children: ReactNode | ((dismiss: () => void) => ReactNode);
  onClose: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const heading = useId();
  const [closing, setClosing] = useState(false);
  const animation = useRef<Animation | null>(null);
  const dismiss = () => {
    if (!animation.current) {
      setClosing(true);
      animation.current = dismissSurface(ref.current, onClose);
    }
  };
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => {
      animation.current?.cancel();
      dialog?.close();
    };
  }, []);
  return (
    <dialog
      ref={ref}
      className="modal"
      aria-labelledby={heading}
      data-closing={closing}
      onCancel={(e) => {
        e.preventDefault();
        dismiss();
      }}
      onClick={(e) => {
        const bounds = e.currentTarget.getBoundingClientRect();
        if (
          e.target === e.currentTarget &&
          (e.clientX < bounds.left ||
            e.clientX > bounds.right ||
            e.clientY < bounds.top ||
            e.clientY > bounds.bottom)
        )
          dismiss();
      }}
    >
      <div className="modal-header">
        <h2 id={heading}>{title}</h2>
        <button
          type="button"
          className="icon-button"
          aria-label="关闭"
          onClick={dismiss}
        >
          <X />
        </button>
      </div>
      <div className="modal-body" inert={closing}>
        {typeof children === "function" ? children(dismiss) : children}
      </div>
    </dialog>
  );
}

import { useEffect, useRef, type ReactNode } from "react";
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
  children: ReactNode;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    return () => ref.current?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className="modal"
      onCancel={onClose}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="modal-header">
        <h2>{title}</h2>
        <button className="icon-button" aria-label="关闭" onClick={onClose}>
          <X />
        </button>
      </div>
      {children}
    </dialog>
  );
}

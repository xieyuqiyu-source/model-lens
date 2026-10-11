import { useCallback, useEffect, useRef } from "react";
import { Info, X } from "lucide-react";
import { dismissSurface } from "./motion";
export function Toast({
  message,
  onClose,
}: {
  message: string;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null),
    animation = useRef<Animation | null>(null),
    callback = useRef(onClose);
  callback.current = onClose;
  const dismiss = useCallback(() => {
    if (!animation.current)
      animation.current = dismissSurface(ref.current, () => callback.current());
  }, []);
  useEffect(() => {
    const timer = setTimeout(dismiss, 4000);
    return () => {
      clearTimeout(timer);
      animation.current?.cancel();
    };
  }, [dismiss]);
  return (
    <div ref={ref} className="toast" role="status">
      <Info size={15} />
      <span>{message}</span>
      <button
        type="button"
        className="icon-button"
        aria-label="关闭提示"
        onClick={dismiss}
      >
        <X size={13} />
      </button>
    </div>
  );
}

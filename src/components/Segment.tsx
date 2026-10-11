export function Segment({
  label,
  value,
  onChange,
  options,
  className = "",
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: readonly [string, string][];
  className?: string;
}) {
  const index = Math.max(
    0,
    options.findIndex(([id]) => id === value),
  );
  return (
    <div
      role="group"
      aria-label={label}
      className={`segment animated-segment ${className}`}
      style={
        {
          "--segment-index": index,
          "--segment-count": options.length,
        } as React.CSSProperties
      }
    >
      {options.map(([id, title]) => (
        <button
          key={id}
          type="button"
          aria-pressed={id === value}
          onClick={() => onChange(id)}
        >
          {title}
        </button>
      ))}
    </div>
  );
}

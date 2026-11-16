import { useId } from "react";

function label(value: string) {
  if (value === "showdown") return "Animated";
  if (value === "home") return "Pokémon HOME";
  if (value.startsWith("generation-"))
    return `Generation ${value.slice(11).toUpperCase()}`;
  const words = value.replaceAll("_", " ").replaceAll("-", " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}
export function SpritePicker({
  name,
  variants,
  value,
  onChange,
  disabled = false,
}: {
  name: string;
  variants: Record<string, string>;
  value?: string | null;
  onChange: (key: string | null) => void;
  disabled?: boolean;
}) {
  const id = useId();
  const groups = new Map<string, { key: string; label: string }[]>();
  for (const key of Object.keys(variants).sort()) {
    if (key === "front_default") continue;
    const parts = key.split("/");
    const group =
      parts.length === 1
        ? "Standard"
        : parts
            .slice(0, -1)
            .filter((part) => part !== "other" && part !== "versions")
            .map(label)
            .join(" / ");
    const options = groups.get(group) || [];
    const name = label(parts.at(-1)!);
    options.push({
      key,
      label: group === "Standard" ? name : `${group} · ${name}`,
    });
    groups.set(group, options);
  }
  const missing = !!value && !variants[value];
  return (
    <div className="sprite-picker">
      <label htmlFor={id}>Sprite for {name}</label>
      <select
        id={id}
        value={value || "front_default"}
        disabled={disabled || (!groups.size && !value)}
        onChange={(event) =>
          onChange(
            event.target.value === "front_default" ? null : event.target.value,
          )
        }
      >
        <option value="front_default">Front default</option>
        {missing && (
          <option value={value!} disabled>
            {label(value!)} (unavailable)
          </option>
        )}
        {[...groups].map(([group, options]) => (
          <optgroup label={group} key={group}>
            {options.map((option) => (
              <option value={option.key} key={option.key}>
                {option.label}
              </option>
            ))}
          </optgroup>
        ))}
      </select>
      {missing && (
        <p className="sprite-help">
          This version is no longer available. Showing the default image.
        </p>
      )}
    </div>
  );
}

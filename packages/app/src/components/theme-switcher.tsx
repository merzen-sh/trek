import { useState, useRef, useEffect } from "react";
import { Moon, Sun, Monitor } from "lucide-react";
import { Button } from "ui";
import { useAppSetting, type Theme } from "../lib/use-app-setting";

const themes: { value: Theme; label: string; icon: typeof Sun }[] = [
  { value: "light", label: "Light", icon: Sun },
  { value: "dark", label: "Dark", icon: Moon },
  { value: "system", label: "System", icon: Monitor },
];

export function ThemeSwitcher() {
  const { theme, setTheme } = useAppSetting();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const current = themes.find((t) => t.value === theme) ?? themes[2];
  const Icon = current.icon;

  return (
    <div ref={ref} className="relative">
      <Button
        variant="outline"
        size="icon"
        onClick={() => setOpen(!open)}
        aria-label="Toggle theme"
      >
        <Icon className="h-4 w-4" />
      </Button>

      {open && (
        <div className="absolute right-0 top-full mt-1 z-50 w-36 rounded-lg border bg-popover p-1 shadow-md">
          {themes.map((t) => {
            const Icon = t.icon;
            return (
              <button
                key={t.value}
                onClick={() => {
                  setTheme(t.value);
                  setOpen(false);
                }}
                className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm ${
                  theme === t.value
                    ? "bg-accent text-accent-foreground"
                    : "hover:bg-accent hover:text-accent-foreground"
                }`}
              >
                <Icon className="h-4 w-4" />
                {t.label}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

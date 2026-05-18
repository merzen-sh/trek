import { useAppSetting } from "../lib/use-app-setting";
import { cn, Card, CardHeader, CardTitle, CardContent } from "ui";

interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
}

export function SettingsDialog({ open, onClose }: SettingsDialogProps) {
  const theme = useAppSetting((s) => s.theme);
  const setTheme = useAppSetting((s) => s.setTheme);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />

      <div className="relative flex w-4xl h-[500px] bg-background border rounded-2xl shadow-xl overflow-hidden">
        <div className="w-[25%] border-r bg-muted/30 p-4">
          <div className="text-sm font-semibold mb-4">Settings</div>
          <nav className="space-y-1">
            <button className="flex w-full items-center rounded-md px-3 py-2 text-sm bg-accent text-accent-foreground">
              Appearance
            </button>
          </nav>
        </div>

        <div className="w-[75%] p-6 flex flex-col gap-4 overflow-auto">
          <div>
            <h2 className="text-lg font-semibold">Appearance</h2>
            <p className="text-sm text-muted-foreground">Customize how the application looks.</p>
          </div>

          <Card>
            <CardHeader className="p-4">
              <CardTitle className="text-sm">Theme Mode</CardTitle>
            </CardHeader>
            <CardContent className="p-4 pt-0">
              <div className="grid grid-cols-3 gap-3">
                <button
                  onClick={() => setTheme("light")}
                  className={cn(
                    "flex flex-col items-center gap-2 p-2 rounded-lg border bg-card text-xs hover:bg-accent transition-all",
                    theme === "light" && "border-primary ring-1 ring-primary",
                  )}
                >
                  <div className="w-full h-14 bg-white border rounded-md p-1.5 flex flex-col gap-1 shadow-sm">
                    <div className="h-2 bg-gray-200 rounded w-3/4" />
                    <div className="h-2 bg-gray-200 rounded w-1/2" />
                    <div className="h-4 bg-blue-100 rounded w-full mt-auto flex items-center justify-center text-[8px] text-blue-600">
                      Button
                    </div>
                  </div>
                  <span className="font-medium">Light</span>
                </button>

                <button
                  onClick={() => setTheme("dark")}
                  className={cn(
                    "flex flex-col items-center gap-2 p-2 rounded-lg border bg-card text-xs hover:bg-accent transition-all",
                    theme === "dark" && "border-primary ring-1 ring-primary",
                  )}
                >
                  <div className="w-full h-14 bg-slate-950 border border-slate-800 rounded-md p-1.5 flex flex-col gap-1 shadow-sm">
                    <div className="h-2 bg-slate-800 rounded w-3/4" />
                    <div className="h-2 bg-slate-800 rounded w-1/2" />
                    <div className="h-4 bg-slate-800 rounded w-full mt-auto flex items-center justify-center text-[8px] text-slate-400">
                      Button
                    </div>
                  </div>
                  <span className="font-medium">Dark</span>
                </button>

                <button
                  onClick={() => setTheme("system")}
                  className={cn(
                    "flex flex-col items-center gap-2 p-2 rounded-lg border bg-card text-xs hover:bg-accent transition-all",
                    theme === "system" && "border-primary ring-1 ring-primary",
                  )}
                >
                  <div className="w-full h-14 bg-gradient-to-br from-white to-slate-950 border rounded-md p-1.5 flex flex-col gap-1 shadow-sm overflow-hidden relative">
                    <div className="absolute inset-0 flex">
                      <div className="w-1/2 bg-white p-1.5 flex flex-col gap-1">
                        <div className="h-2 bg-gray-200 rounded w-3/4" />
                        <div className="h-2 bg-gray-200 rounded w-1/2" />
                      </div>
                      <div className="w-1/2 bg-slate-950 p-1.5 flex flex-col gap-1">
                        <div className="h-2 bg-slate-800 rounded w-3/4" />
                        <div className="h-2 bg-slate-800 rounded w-1/2" />
                      </div>
                    </div>
                    <div className="h-4 bg-gray-500/20 rounded w-full mt-auto flex items-center justify-center text-[8px] text-white/70 z-10">
                      Button
                    </div>
                  </div>
                  <span className="font-medium">System</span>
                </button>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}

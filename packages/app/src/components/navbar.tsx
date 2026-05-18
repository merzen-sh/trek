import { PanelLeftClose, PanelLeft } from "lucide-react"
import { Button } from "ui"
import { useAppSetting } from "../lib/use-app-setting"
import { ThemeSwitcher } from "./theme-switcher"

export function Navbar() {
  const sidebarOpen = useAppSetting((s) => s.sidebarOpen)
  const toggleSidebar = useAppSetting((s) => s.toggleSidebar)

  return (
    <header className="flex h-12 items-center justify-between border-b px-4">
      <Button variant="ghost" size="icon" onClick={toggleSidebar}>
        {sidebarOpen ? (
          <PanelLeftClose className="h-4 w-4" />
        ) : (
          <PanelLeft className="h-4 w-4" />
        )}
      </Button>

      <ThemeSwitcher />
    </header>
  )
}

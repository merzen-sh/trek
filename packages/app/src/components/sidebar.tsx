import { useState } from "react"
import {
  Monitor,
  Database,
  Terminal,
  Globe,
} from "lucide-react"
import { useAppSetting } from "../lib/use-app-setting"
import { cn } from "ui"

interface Resource {
  id: string
  name: string
  icon: typeof Monitor
  menus: { label: string }[]
}

const resources: Resource[] = [
  {
    id: "server",
    name: "Display Server",
    icon: Monitor,
    menus: [
      { label: "Overview" },
      { label: "Logs" },
      { label: "Metrics" },
      { label: "Settings" },
    ],
  },
  {
    id: "database",
    name: "Database",
    icon: Database,
    menus: [
      { label: "Tables" },
      { label: "Queries" },
      { label: "Backups" },
      { label: "Config" },
    ],
  },
  {
    id: "terminal",
    name: "Terminal",
    icon: Terminal,
    menus: [
      { label: "Sessions" },
      { label: "Commands" },
      { label: "History" },
    ],
  },
  {
    id: "globe",
    name: "Network",
    icon: Globe,
    menus: [
      { label: "Endpoints" },
      { label: "Routes" },
      { label: "Firewall" },
      { label: "DNS" },
    ],
  },
]

function SidebarContent({
  selected,
  onSelect,
  sidebarOpen,
}: {
  selected: string
  onSelect: (id: string) => void
  sidebarOpen: boolean
}) {
  const active = resources.find((r) => r.id === selected)

  return (
    <>
      <aside className="flex w-16 flex-shrink-0 flex-col items-center gap-3 border-r bg-muted/50 py-3">
        {resources.map((r) => (
          <button
            key={r.id}
            onClick={() => onSelect(r.id)}
            className={cn(
              "flex h-9 w-9 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:rounded-lg hover:bg-primary hover:text-primary-foreground",
              selected === r.id && "rounded-lg bg-primary text-primary-foreground",
            )}
            title={r.name}
          >
            <r.icon className="h-4 w-4" />
          </button>
        ))}
      </aside>

      <aside
        className={cn(
          "overflow-hidden border-r bg-muted/20 transition-all duration-200",
          sidebarOpen ? "w-60" : "w-0",
        )}
      >
        {active && (
          <div className="w-60 shrink-0">
            <div className="flex h-12 items-center border-b px-4 text-sm font-semibold">
              {active.name}
            </div>
            <nav className="space-y-0.5 p-2">
              {active.menus.map((m) => (
                <button
                  key={m.label}
                  className="flex w-full items-center rounded-md px-3 py-1.5 text-sm text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                >
                  {m.label}
                </button>
              ))}
            </nav>
          </div>
        )}
      </aside>
    </>
  )
}

export function Sidebar() {
  const sidebarOpen = useAppSetting((s) => s.sidebarOpen)
  const setSidebarOpen = useAppSetting((s) => s.setSidebarOpen)
  const [selected, setSelected] = useState(resources[0].id)

  function handleSelect(id: string) {
    setSelected(id)
    if (!sidebarOpen) setSidebarOpen(true)
  }

  return (
    <>
      {/* Mobile overlay — starts below navbar so toggle stays reachable */}
      <div
        className={cn(
          "fixed inset-x-0 top-12 bottom-0 z-40 lg:hidden",
          sidebarOpen ? "pointer-events-auto" : "pointer-events-none",
        )}
        onClick={() => setSidebarOpen(false)}
      >
        <div
          className={cn(
            "absolute inset-0 bg-black/50 transition-opacity duration-200",
            sidebarOpen ? "opacity-100" : "opacity-0",
          )}
        />
        <div
          className={cn(
            "relative flex h-full transition-transform duration-200",
            sidebarOpen ? "translate-x-0" : "-translate-x-full",
          )}
          onClick={(e) => e.stopPropagation()}
        >
          <SidebarContent
            selected={selected}
            onSelect={handleSelect}
            sidebarOpen={true}
          />
        </div>
      </div>

      {/* Desktop inline */}
      <div className="hidden lg:flex">
        <SidebarContent
          selected={selected}
          onSelect={handleSelect}
          sidebarOpen={sidebarOpen}
        />
      </div>
    </>
  )
}

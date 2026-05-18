import { useEffect } from "react";
import { Outlet } from "@tanstack/react-router";
import { useAppSetting } from "../lib/use-app-setting";
import { Navbar } from "../components/navbar";
import { Sidebar } from "../components/sidebar";

function resolveTheme(theme: "light" | "dark" | "system"): "light" | "dark" {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }
  return theme;
}

function applyTheme(resolved: "light" | "dark") {
  const root = document.documentElement;
  root.classList.remove("light", "dark");
  root.classList.add(resolved);
}

function applyThemeSmooth(resolved: "light" | "dark") {
  const root = document.documentElement;
  root.classList.add("disable-transition");
  applyTheme(resolved);
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      root.classList.remove("disable-transition");
    });
  });
}

export function RootLayout() {
  const theme = useAppSetting((s) => s.theme);
  const sidebarOpen = useAppSetting((s) => s.sidebarOpen);
  const setSidebarOpen = useAppSetting((s) => s.setSidebarOpen);

  useEffect(() => {
    applyThemeSmooth(resolveTheme(theme));
  }, [theme]);

  useEffect(() => {
    if (theme !== "system") return;

    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => applyThemeSmooth(e.matches ? "dark" : "light");
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  return (
    <div className="flex h-screen flex-col">
      <Navbar />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main
          className="flex-1 overflow-auto"
          onClick={() => {
            if (sidebarOpen) setSidebarOpen(false);
          }}
        >
          <Outlet />
        </main>
      </div>
    </div>
  );
}

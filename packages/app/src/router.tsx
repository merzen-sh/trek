import { createRootRoute, createRoute, createRouter } from "@tanstack/react-router";
import { RootLayout } from "./routes/__root";
import { HomePage } from "./routes/index";
import { ConverterPage } from "./routes/converter";

const rootRoute = createRootRoute({
  component: RootLayout,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: HomePage,
});

const convertRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/converter",
  component: ConverterPage,
});

const routeTree = rootRoute.addChildren([indexRoute, convertRoute]);

export const router = createRouter({ routeTree });

import { spawnSync } from "node:child_process";

const result = spawnSync("cargo", ["build", "--release"], {
  stdio: "inherit",
  cwd: new URL("..", import.meta.url).pathname,
  env: {
    ...process.env,
  },
});

process.exit(result.status ?? 1);

import { spawn } from "node:child_process";
import { rm } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = join(scriptDir, "..");
const minPort = Number.parseInt(process.env.PORT ?? "3001", 10);

async function removeCache() {
  await rm(join(root, ".next"), { recursive: true, force: true });
  await rm(join(root, "tsconfig.tsbuildinfo"), { force: true });
}

async function portInUse(port) {
  return await new Promise((resolve) => {
    const probe = spawn("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN"], {
      stdio: "ignore",
    });
    probe.on("exit", (code) => resolve(code === 0));
    probe.on("error", () => resolve(false));
  });
}

async function pickPort(startPort) {
  let port = startPort;
  while (await portInUse(port)) {
    port += 1;
  }
  return port;
}

await removeCache();

const port = await pickPort(minPort);
if (port !== minPort) {
  console.log(`WARN: dashboard port :${minPort} is busy; using :${port}`);
}

const child = spawn(join(root, "node_modules/.bin/next"), ["dev", "-p", String(port)], {
  cwd: root,
  stdio: "inherit",
  env: {
    ...process.env,
    PORT: String(port),
  },
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});

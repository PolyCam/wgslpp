#!/usr/bin/env node
// Launcher for the `wgslpp` bin entry. Resolves the platform-specific
// binary that npm installed via `optionalDependencies` and execs it with
// the user's argv. Stdio is inherited so output streams through unchanged.

import { spawnSync } from "node:child_process";

import { resolveBinary } from "../binary.js";

let binPath;
try {
  binPath = resolveBinary();
} catch (err) {
  console.error(err.message);
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: true,
});

if (result.error) {
  console.error(`wgslpp: failed to spawn ${binPath}: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);

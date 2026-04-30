// Resolve the platform-specific wgslpp binary path.
//
// `optionalDependencies` in package.json declares the three platform sub-
// packages with `os`/`cpu` constraints. npm installs only the matching one;
// this helper finds whichever did get installed (or honours an env override
// for development).

import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

const TARGETS = {
  "darwin-arm64": "@polycam/wgslpp-darwin-arm64",
  "linux-x64": "@polycam/wgslpp-linux-x64",
  "win32-x64": "@polycam/wgslpp-win32-x64",
};

/**
 * Locate the wgslpp binary. Resolution order:
 *
 *   1. `WGSLPP_BIN` env var (absolute path) — wins over everything, useful
 *      for pointing at a locally-built `target/release/wgslpp` during dev.
 *   2. The platform sub-package matching `process.platform` /
 *      `process.arch` — what users get when they install the published
 *      meta package.
 *
 * Throws a descriptive Error if neither is available, rather than letting
 * the spawn fail with an unhelpful ENOENT.
 *
 * @returns {string} Absolute path to the wgslpp executable.
 */
export function resolveBinary() {
  if (process.env.WGSLPP_BIN) {
    return process.env.WGSLPP_BIN;
  }

  const key = `${process.platform}-${process.arch}`;
  const pkg = TARGETS[key];
  if (!pkg) {
    throw new Error(
      `wgslpp: unsupported platform/arch combination ${key}. ` +
        `Supported: ${Object.keys(TARGETS).join(", ")}.`,
    );
  }

  const exe = process.platform === "win32" ? "wgslpp.exe" : "wgslpp";
  const target = `${pkg}/bin/${exe}`;

  // Primary resolution: from this module's own location. This is the
  // common case — npm install puts the platform package as a sibling in
  // the same node_modules tree.
  try {
    return require.resolve(target);
  } catch {
    // Fall through.
  }

  // Fallback: resolve from the consumer's cwd. This catches Yarn `portal:`
  // local-dev setups where the meta package is symlinked into a different
  // node_modules tree than the platform package, so module-relative
  // resolution can't bridge them. Without this, callers would need
  // `node --preserve-symlinks`.
  try {
    const cwdRequire = createRequire(`${process.cwd()}/`);
    return cwdRequire.resolve(target);
  } catch {
    throw new Error(
      `wgslpp: platform package '${pkg}' is not installed. ` +
        `This usually means npm skipped optional dependencies — re-run ` +
        `install with --include=optional, or set WGSLPP_BIN to a locally ` +
        `built binary.`,
    );
  }
}

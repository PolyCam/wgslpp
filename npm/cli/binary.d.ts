/**
 * Locate the wgslpp binary. Resolution order:
 *
 *   1. `WGSLPP_BIN` env var (absolute path) — wins over everything, useful
 *      for pointing at a locally-built `target/release/wgslpp` during dev.
 *   2. The platform sub-package matching `process.platform` /
 *      `process.arch` — what users get when they install the published
 *      meta package.
 *
 * Throws a descriptive Error if neither is available.
 */
export declare function resolveBinary(): string;

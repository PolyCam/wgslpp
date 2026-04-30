// Public API for `@polycam/wgslpp`.
//
// The npm package primarily ships a binary (`wgslpp` on PATH after install),
// but Node consumers — typically build scripts — can also import these
// helpers to drive the subcommands without rolling their own subprocess +
// JSON parsing.
//
// Types live alongside in `index.d.ts` (hand-maintained, no build step).
// Keep the two in sync when editing.

import { execFile, execFileSync } from "node:child_process";
import { promisify } from "node:util";

import { resolveBinary } from "./binary.js";

export { resolveBinary };

const execFileAsync = promisify(execFile);

// ── Type guards ──────────────────────────────────────────────────────────────
//
// Also declared in index.d.ts as `is X` narrowing predicates so TypeScript
// callers get the discriminated-union benefit. The runtime check is a plain
// string comparison.

export function isBuffer(b) {
  return (
    b.type === "uniform" ||
    b.type === "storage_read" ||
    b.type === "storage_read_write"
  );
}

export function isSampler(b) {
  return b.type === "sampler" || b.type === "sampler_comparison";
}

export function isTexture(b) {
  return b.type === "texture";
}

// ── Subprocess helpers ───────────────────────────────────────────────────────
//
// Every subcommand ultimately boils down to "spawn wgslpp with these args,
// capture stdout/stderr, look at the exit code, parse output". These helpers
// centralise the spawn so each subcommand wrapper is just argv-building +
// output handling.

const MAX_BUFFER = 64 * 1024 * 1024;

class WgslppError extends Error {
  constructor(message, { stderr, exitCode } = {}) {
    super(message);
    this.name = "WgslppError";
    if (stderr !== undefined) this.stderr = stderr;
    if (exitCode !== undefined) this.exitCode = exitCode;
  }
}

function runSync(bin, args, { input } = {}) {
  // execFileSync throws on non-zero exit; we want to inspect both streams
  // first (validate emits diagnostics on stderr even on error). Use spawnSync
  // shape via execFileSync's error object instead.
  try {
    const stdout = execFileSync(bin, args, {
      input,
      encoding: "utf-8",
      stdio: ["pipe", "pipe", "pipe"],
      maxBuffer: MAX_BUFFER,
    });
    return { stdout, stderr: "", exitCode: 0 };
  } catch (err) {
    if (typeof err.status === "number") {
      return {
        stdout: err.stdout?.toString("utf-8") ?? "",
        stderr: err.stderr?.toString("utf-8") ?? "",
        exitCode: err.status,
      };
    }
    throw err;
  }
}

async function runAsync(bin, args, { input } = {}) {
  try {
    const { stdout, stderr } = await execFileAsync(bin, args, {
      encoding: "utf-8",
      maxBuffer: MAX_BUFFER,
      input,
    });
    return { stdout, stderr, exitCode: 0 };
  } catch (err) {
    if (typeof err.code === "number") {
      return { stdout: err.stdout ?? "", stderr: err.stderr ?? "", exitCode: err.code };
    }
    throw err;
  }
}

// ── Argument builders ────────────────────────────────────────────────────────

function preprocessArgs(options) {
  const args = ["preprocess"];
  if (options.input) args.push(options.input);
  if (options.config) args.push("--config", options.config);
  for (const [name, path] of options.packages ?? []) {
    args.push("-P", `${name}=${path}`);
  }
  for (const d of options.defines ?? []) {
    args.push("-D", d);
  }
  if (options.sourceMap) args.push("--source-map", options.sourceMap);
  if (options.filePath) args.push("--file-path", options.filePath);
  return args;
}

function validateArgs(options) {
  const args = ["validate", options.input, "--format", "json"];
  if (options.sourceMap) args.push("--source-map", options.sourceMap);
  return args;
}

function reflectArgs(options) {
  const args = ["reflect"];
  if (options.input) args.push(options.input);
  return args;
}

function minifyArgs(options) {
  const args = ["minify", options.input];
  if (options.dce) args.push("--dce");
  if (options.rename) args.push("--rename");
  return args;
}

function pipelineArgs(options) {
  const args = ["pipeline", "--input", options.input];
  if (options.config) args.push("--config", options.config);
  for (const [name, path] of options.packages ?? []) {
    args.push("-P", `${name}=${path}`);
  }
  for (const d of options.defines ?? []) {
    args.push("-D", d);
  }
  if (options.minify) args.push("--minify");
  if (options.dce) args.push("--dce");
  if (options.rename) args.push("--rename");
  if (options.noValidate) args.push("--no-validate");
  return args;
}

// ── Result handlers ──────────────────────────────────────────────────────────

function ensureSuccess(subcommand, { stderr, exitCode }) {
  if (exitCode !== 0) {
    throw new WgslppError(
      `wgslpp ${subcommand} failed (exit ${exitCode}): ${stderr.trim()}`,
      { stderr, exitCode },
    );
  }
}

function parseJson(subcommand, raw) {
  try {
    return JSON.parse(raw);
  } catch (err) {
    throw new WgslppError(
      `wgslpp ${subcommand}: failed to parse JSON output: ${err.message}`,
      { stderr: raw },
    );
  }
}

function preprocessResult(result) {
  ensureSuccess("preprocess", result);
  return { code: result.stdout };
}

// Validate is special: diagnostics go to *stderr* even on success ("Valid."
// prose) or failure (JSON array). Exit 0 = no errors; exit !=0 = errors.
// Either way, we want a structured `{ valid, diagnostics }` back.
function validateResult(result) {
  const { stderr, exitCode } = result;
  if (exitCode === 0) {
    return { valid: true, diagnostics: [] };
  }
  const trimmed = stderr.trim();
  // Find the JSON payload: it starts with `[` and is followed by an
  // "error: validation failed" trailer that the CLI emits unconditionally.
  const start = trimmed.indexOf("[");
  if (start === -1) {
    throw new WgslppError(
      `wgslpp validate failed (exit ${exitCode}): ${trimmed}`,
      { stderr, exitCode },
    );
  }
  // Walk forward to find the matching `]`. The JSON array is well-formed
  // so a simple bracket counter works (strings can't contain unescaped `]`
  // by JSON spec).
  let depth = 0;
  let end = -1;
  let inString = false;
  let escape = false;
  for (let i = start; i < trimmed.length; i++) {
    const c = trimmed[i];
    if (escape) {
      escape = false;
      continue;
    }
    if (inString) {
      if (c === "\\") escape = true;
      else if (c === '"') inString = false;
      continue;
    }
    if (c === '"') inString = true;
    else if (c === "[") depth++;
    else if (c === "]") {
      depth--;
      if (depth === 0) {
        end = i;
        break;
      }
    }
  }
  if (end === -1) {
    throw new WgslppError(
      `wgslpp validate: malformed JSON in stderr: ${trimmed}`,
      { stderr, exitCode },
    );
  }
  const diagnostics = parseJson("validate", trimmed.slice(start, end + 1));
  return { valid: false, diagnostics };
}

function reflectResult(result) {
  ensureSuccess("reflect", result);
  return parseJson("reflect", result.stdout);
}

function minifyResult(result) {
  ensureSuccess("minify", result);
  return { code: result.stdout };
}

function pipelineResult(result) {
  ensureSuccess("pipeline", result);
  return parseJson("pipeline", result.stdout);
}

// ── Public API ───────────────────────────────────────────────────────────────

function bin(options) {
  return options?.binPath ?? resolveBinary();
}

/** Run `wgslpp preprocess` synchronously. */
export function preprocess(options) {
  return preprocessResult(runSync(bin(options), preprocessArgs(options)));
}

/** Async variant of `preprocess`. */
export async function preprocessAsync(options) {
  return preprocessResult(await runAsync(bin(options), preprocessArgs(options)));
}

/** Run `wgslpp validate`. Always returns `{ valid, diagnostics }`; only
 *  throws on subprocess crashes, not on validation errors. */
export function validate(options) {
  return validateResult(runSync(bin(options), validateArgs(options)));
}

/** Async variant of `validate`. */
export async function validateAsync(options) {
  return validateResult(await runAsync(bin(options), validateArgs(options)));
}

/** Run `wgslpp reflect` and return the parsed reflection JSON. */
export function reflect(options) {
  return reflectResult(runSync(bin(options), reflectArgs(options)));
}

/** Async variant of `reflect`. */
export async function reflectAsync(options) {
  return reflectResult(await runAsync(bin(options), reflectArgs(options)));
}

/** Run `wgslpp minify` and return the minified WGSL. */
export function minify(options) {
  return minifyResult(runSync(bin(options), minifyArgs(options)));
}

/** Async variant of `minify`. */
export async function minifyAsync(options) {
  return minifyResult(await runAsync(bin(options), minifyArgs(options)));
}

/** Run `wgslpp pipeline` and return the parsed JSON. */
export function pipeline(options) {
  return pipelineResult(runSync(bin(options), pipelineArgs(options)));
}

/** Async variant of `pipeline`. */
export async function pipelineAsync(options) {
  return pipelineResult(await runAsync(bin(options), pipelineArgs(options)));
}

export { WgslppError };

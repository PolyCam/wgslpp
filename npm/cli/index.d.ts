// Type declarations for `@polycam/wgslpp`.
//
// These mirror `crates/wgslpp-core/src/{reflect,validate}.rs`. The Rust side
// produces a single `unfilterable` / `nonfiltering` / `dynamic_offset` flag
// space across all bindings; on the TypeScript side we tighten that into a
// discriminated union so each kind only exposes the flags that apply.
//
// Hand-maintained alongside index.js — keep them in sync when editing.

export { resolveBinary } from "./binary.js";

// ── Reflection types ─────────────────────────────────────────────────────────

export interface FieldInfo {
  name: string;
  type: string;
  offset: number;
  size: number;
}

export interface StructInfo {
  name: string;
  size: number;
  alignment: number;
  fields: FieldInfo[];
}

/** The buffer-style binding kinds. */
export type BufferKind = "uniform" | "storage_read" | "storage_read_write";

/** The sampler-style binding kinds. */
export type SamplerKind = "sampler" | "sampler_comparison";

/** The full set of `BindingInfo.type` discriminators. */
export type BindingKind = BufferKind | SamplerKind | "texture";

/** Fields shared by every `BindingInfo` shape, regardless of kind. */
interface BindingBase {
  group: number;
  binding: number;
  name: string;
  /**
   * The WGSL type that follows the colon in the var declaration:
   *   - buffers:  the struct name (e.g. `FrameUniforms`)
   *   - textures: the full texture type (e.g. `texture_2d<f32>`)
   *   - samplers: `sampler` or `sampler_comparison`
   */
  wgsl_type: string;
}

/**
 * A `var<uniform>` / `var<storage, ...>` binding. `dynamic_offset` is the
 * only kind-specific flag and only meaningful for buffers.
 */
export type BufferBinding = BindingBase & {
  type: BufferKind;
  /** Set by the `/// @dynamic_offset` marker comment. */
  dynamic_offset?: boolean;
};

/**
 * A sampler binding. Only the plain `sampler` variant supports the
 * `nonfiltering` override; `sampler_comparison` carries no flags.
 */
export type SamplerBinding =
  | (BindingBase & {
      type: "sampler";
      /** Set by the `/// @nonfiltering` marker comment. */
      nonfiltering?: boolean;
    })
  | (BindingBase & {
      type: "sampler_comparison";
    });

/**
 * A texture binding. `unfilterable` is set either explicitly (via the
 * `/// @unfilterable` marker) or implicitly when the texture is multi-
 * sampled (the WebGPU spec makes those intrinsically unfilterable).
 */
export type TextureBinding = BindingBase & {
  type: "texture";
  unfilterable?: boolean;
};

/**
 * Discriminated union over all binding kinds. Use the type guards
 * (`isBuffer`, `isSampler`, `isTexture`) to narrow.
 */
export type BindingInfo = BufferBinding | SamplerBinding | TextureBinding;

export interface EntryPointInfo {
  name: string;
  stage: "vertex" | "fragment" | "compute" | "task" | "mesh";
  workgroup_size?: [number, number, number];
}

export interface ReflectionData {
  bindings: BindingInfo[];
  structs: StructInfo[];
  entry_points: EntryPointInfo[];
}

// ── Type guards ──────────────────────────────────────────────────────────────

/** Narrow to a buffer binding (uniform, storage, storage RW). */
export declare function isBuffer(b: BindingInfo): b is BufferBinding;

/** Narrow to a sampler binding (filterable or comparison). */
export declare function isSampler(b: BindingInfo): b is SamplerBinding;

/** Narrow to a texture binding. */
export declare function isTexture(b: BindingInfo): b is TextureBinding;

// ── Diagnostic types ─────────────────────────────────────────────────────────

export type Severity = "error" | "warning";

export interface Diagnostic {
  severity: Severity;
  message: string;
  /** Original file path, when source maps remapped the location. */
  file: string | null;
  /** 1-based line number. */
  line: number | null;
  /** 1-based column. */
  column: number | null;
  /** Additional context lines from naga's error chain. */
  notes: string[];
}

// ── Common option shapes ─────────────────────────────────────────────────────

interface SubcommandOptions {
  /**
   * Override the binary used. Defaults to `WGSLPP_BIN` env or the bundled
   * platform binary.
   */
  binPath?: string;
}

// ── Subcommand options + outputs ─────────────────────────────────────────────

export interface PreprocessOptions extends SubcommandOptions {
  /** Input WGSL file path. */
  input: string;
  /** `wgslpp.json` config for package resolution. Optional. */
  config?: string;
  /** `-D` flags. Each entry is `KEY` or `KEY=VAL`. */
  defines?: string[];
  /**
   * `-P` flags. Each entry is `[name, path]` mapping a package include
   * prefix to a directory.
   */
  packages?: Array<[string, string]>;
  /** Write the source map to this path. */
  sourceMap?: string;
  /**
   * Virtual file path to use for include resolution (matters when reading
   * source from a buffer rather than a real file).
   */
  filePath?: string;
}

export interface PreprocessOutput {
  /** Preprocessed WGSL source. */
  code: string;
}

export interface ValidateOptions extends SubcommandOptions {
  /** Input WGSL file path (already preprocessed, or raw). */
  input: string;
  /**
   * Optional source map JSON file path, used to remap diagnostic
   * locations back to original source files.
   */
  sourceMap?: string;
}

export interface ValidateOutput {
  /** True iff the file parses and validates clean. */
  valid: boolean;
  /** Diagnostics emitted during validation; empty when `valid` is true. */
  diagnostics: Diagnostic[];
}

export interface ReflectOptions extends SubcommandOptions {
  /** Input WGSL file path. */
  input: string;
}

export interface MinifyOptions extends SubcommandOptions {
  /** Input WGSL file path. */
  input: string;
  /** Eliminate dead code before writing. */
  dce?: boolean;
  /** Frequency-based identifier renaming before writing. */
  rename?: boolean;
}

export interface MinifyOutput {
  /** Minified WGSL source. */
  code: string;
}

export interface PipelineOptions extends SubcommandOptions {
  /** Input WGSL file path. */
  input: string;
  /** `wgslpp.json` config for package resolution. Optional. */
  config?: string;
  /** `-D` flags. Each entry is `KEY` or `KEY=VAL`. */
  defines?: string[];
  /**
   * `-P` flags. Each entry is `[name, path]` mapping a package include
   * prefix to a directory.
   */
  packages?: Array<[string, string]>;
  /**
   * Minify the embedded `code` field (run the WGSL output through naga's
   * writer).
   */
  minify?: boolean;
  /** Eliminate dead code before minify. No-op without `minify`. */
  dce?: boolean;
  /**
   * Frequency-based identifier renaming before minify. No-op without
   * `minify`.
   */
  rename?: boolean;
  /** Skip validation; still parses (reflection requires a parsed module). */
  noValidate?: boolean;
}

export interface PipelineOutput {
  /** Final WGSL source — minified when `minify` is set, else preprocessed. */
  code: string;
  /** Final `#define` table after preprocessing. */
  defines: Record<string, string>;
  /** Reflection from the parsed (pre-minify) module. */
  reflection: ReflectionData;
}

// ── Errors ───────────────────────────────────────────────────────────────────

/**
 * Thrown when a subcommand exits non-zero (other than `validate`, which
 * surfaces validation failures via `ValidateOutput.valid`). Carries the
 * captured stderr and exit code so callers can inspect or forward.
 */
export declare class WgslppError extends Error {
  readonly stderr?: string;
  readonly exitCode?: number;
}

// ── API surface ──────────────────────────────────────────────────────────────

export declare function preprocess(options: PreprocessOptions): PreprocessOutput;
export declare function preprocessAsync(
  options: PreprocessOptions,
): Promise<PreprocessOutput>;

/**
 * Run `wgslpp validate`. Unlike the CLI, this never throws on validation
 * errors — it returns `{ valid: false, diagnostics: [...] }` so callers
 * can decide how to react. It only throws for subprocess-level failures
 * (binary missing, crash, malformed output).
 */
export declare function validate(options: ValidateOptions): ValidateOutput;
export declare function validateAsync(
  options: ValidateOptions,
): Promise<ValidateOutput>;

export declare function reflect(options: ReflectOptions): ReflectionData;
export declare function reflectAsync(
  options: ReflectOptions,
): Promise<ReflectionData>;

export declare function minify(options: MinifyOptions): MinifyOutput;
export declare function minifyAsync(
  options: MinifyOptions,
): Promise<MinifyOutput>;

export declare function pipeline(options: PipelineOptions): PipelineOutput;
export declare function pipelineAsync(
  options: PipelineOptions,
): Promise<PipelineOutput>;

# wgslpp

A Rust toolchain for WGSL shaders: `#include`/`#define`/`#pragma once` preprocessing, naga-based validation, dead code elimination, identifier renaming, reflection, and a full LSP server.

## Overview

wgslpp consolidates WGSL shader preprocessing, validation, optimization, and editor tooling into a single binary. The CLI exposes individual stages or an all-in-one `pipeline` subcommand for build integration. The LSP server provides live diagnostics, go-to-definition, hover, completion, semantic highlighting, formatting, and more. Editor plugins are included for both VS Code and CLion/IntelliJ.

## Install

### npm (recommended)

The primary distribution channel. Installs a prebuilt native binary for your platform and a typed Node API.

```sh
# global
npm install -g @polycam/wgslpp

# per-project
npm install --save-dev @polycam/wgslpp
```

Supported platforms: macOS arm64, Linux x86_64, Windows x86_64. The right binary is pulled in automatically via `optionalDependencies`. See [`npm/cli/README.md`](npm/cli/README.md) for the package details.

### GitHub Releases

Every tagged release publishes prebuilt binaries and editor extensions on the [Releases page](https://github.com/polycam/wgslpp/releases). Useful when you don't want npm in the loop or when shipping the binaries through your own packaging.

| Asset                                    | Contents                                                     |
| ---------------------------------------- | ------------------------------------------------------------ |
| `wgslpp-aarch64-apple-darwin.tar.gz`     | `wgslpp` + `wgslpp-lsp` for macOS arm64                      |
| `wgslpp-x86_64-unknown-linux-gnu.tar.gz` | `wgslpp` + `wgslpp-lsp` for Linux x86_64                     |
| `wgslpp-x86_64-pc-windows-msvc.tar.gz`   | `wgslpp.exe` + `wgslpp-lsp.exe` for Windows x86_64           |
| `wgslpp-darwin-arm64.vsix`               | VS Code extension with bundled `wgslpp-lsp` (macOS arm64)    |
| `wgslpp-linux-x64.vsix`                  | VS Code extension with bundled `wgslpp-lsp` (Linux x86_64)   |
| `wgslpp-win32-x64.vsix`                  | VS Code extension with bundled `wgslpp-lsp` (Windows x86_64) |

Install the `.vsix` directly via **Extensions → "..." → Install from VSIX...** in VS Code, or via `code --install-extension wgslpp-<target>.vsix` on the command line.

### From source

```sh
cargo build --release
```

Produces two binaries in `target/release/`:

- **`wgslpp`** — CLI tool
- **`wgslpp-lsp`** — Language server (stdio transport)

## CLI

### `wgslpp preprocess`

Resolve `#include`, `#define`, `#ifdef`, and other preprocessor directives.

```
wgslpp preprocess <input> [-P <name=path>]... [-D <name[=val]>]... [-o output] [--source-map file]
```

| Flag                  | Description                                                                    |
| --------------------- | ------------------------------------------------------------------------------ |
| `-P <name=path>`      | Named package mapping (repeatable). Maps `#include <name/...>` to a directory. |
| `-D <name[=value]>`   | Define a preprocessor macro (repeatable)                                       |
| `-o, --output <path>` | Output file (default: stdout)                                                  |
| `--source-map <path>` | Write source map JSON                                                          |

### `wgslpp validate`

Parse and validate WGSL using naga.

```
wgslpp validate <input> [--source-map file] [--format human|json|gcc]
```

| Flag                  | Description                                                |
| --------------------- | ---------------------------------------------------------- |
| `--source-map <path>` | Remap error locations through a source map                 |
| `--format <format>`   | Diagnostic output format: `human` (default), `json`, `gcc` |

### `wgslpp reflect`

Extract bindings, structs, and entry points as JSON.

```
wgslpp reflect <input> [-o output.json]
```

### `wgslpp minify`

Minify WGSL via the naga writer, optionally with DCE and renaming.

```
wgslpp minify <input> [-o output] [--dce] [--rename]
```

| Flag       | Description                                          |
| ---------- | ---------------------------------------------------- |
| `--dce`    | Remove unreachable functions (dead code elimination) |
| `--rename` | Frequency-based identifier shortening                |

### `wgslpp pipeline`

All-in-one: preprocess → validate → reflect → minify.

```
wgslpp pipeline --input <input> [-P <name=path>]... [-D <name[=val]>]... [-o output] \
    [--config wgslpp.json] [--source-map file] [--no-validate] [--minify] [--dce] [--rename]
```

This is the primary build integration point — a single invocation replaces separate preprocessing, validation, reflection, and optimization steps.

## Node API

`@polycam/wgslpp` ships a typed Node API alongside the CLI binary. Every subcommand has a matching exported function (sync + async); `pipeline()` is the canonical entry point for build scripts.

```ts
import {
  pipeline,
  preprocess,
  validate,
  reflect,
  minify,
  isTexture,
  isSampler,
  isBuffer,
} from "@polycam/wgslpp";

const out = pipeline({
  input: "shaders/main.wgsl",
  config: "wgslpp.json",
  defines: ["USE_SHADOWS=1"],
  minify: true,
  dce: true,
});

console.log(out.code); // final WGSL source
console.log(out.defines); // resolved #define table

for (const b of out.reflection.bindings) {
  if (isTexture(b)) {
    // b.unfilterable is in scope here; b.dynamic_offset is a type error.
  } else if (isSampler(b) && b.type === "sampler") {
    // b.nonfiltering is in scope here.
  } else if (isBuffer(b)) {
    // b.dynamic_offset is in scope here.
  }
}
```

`BindingInfo` is a discriminated union over `BufferBinding`, `SamplerBinding`, and `TextureBinding`. Each kind only exposes the marker flags that apply to it — there's no way to ask a sampler if it's `unfilterable`.

`validate()` never throws on validation errors — it returns `{ valid: false, diagnostics }` so callers decide how to react. Subprocess-level failures (binary missing, output unparseable) throw a `WgslppError` with `stderr` and `exitCode` attached.

The package requires Node ≥ 22 (current LTS). Binary discovery resolves in this order: `binPath` option → `WGSLPP_BIN` env → matching platform sub-package.

## Preprocessor Directives

| Directive                      | Description                                                        |
| ------------------------------ | ------------------------------------------------------------------ |
| `#include "path.wgsl"`         | Textual inclusion, relative to the current file                    |
| `#include <pkg/path.wgsl>`     | Package-scoped inclusion (resolved via `-P` flag or `wgslpp.json`) |
| `#pragma once`                 | Include this file at most once                                     |
| `#define NAME`                 | Define a flag                                                      |
| `#define NAME value`           | Object-like macro (text replacement)                               |
| `#define NAME(a, b) body`      | Function-like macro with parameter substitution                    |
| `#undef NAME`                  | Remove a definition                                                |
| `#ifdef NAME` / `#ifndef NAME` | Conditional on definition existence                                |
| `#if EXPR` / `#elif EXPR`      | Conditional on expression value                                    |
| `#else` / `#endif`             | Conditional block control                                          |

### Expression Grammar

`#if` and `#elif` support a C-like expression language. Numbers can be decimal or hexadecimal (`0x1F`). Identifiers are recursively resolved through macro definitions (e.g. `BRDF_SPECULAR_D` → `SPECULAR_D_GGX` → `0`).

```
expr       = or_expr
or_expr    = and_expr ("||" and_expr)*
and_expr   = bitor_expr ("&&" bitor_expr)*
bitor_expr = cmp_expr ("|" cmp_expr)*
cmp_expr   = bitand_expr (("==" | "!=" | "<" | ">" | "<=" | ">=") bitand_expr)?
bitand_expr = unary_expr ("&" unary_expr)*
unary_expr = "!" unary_expr | primary
primary    = "defined" "(" IDENT ")" | "(" expr ")" | NUMBER | HEX | IDENT
```

### Package-Scoped Includes

Register named packages via `-P` on the CLI or in `wgslpp.json`:

```sh
wgslpp pipeline --input main.wgsl -P polymer=./polymer/shaders -P common=./shared/shaders
```

Then in WGSL:

```wgsl
#include <polymer/lighting/pbr.wgsl>
#include <common/math.wgsl>
```

`#include <polymer/lighting/pbr.wgsl>` resolves to `./polymer/shaders/lighting/pbr.wgsl`.

## Binding Marker Comments

Place a `///` doc comment immediately before a `var` declaration to attach metadata that surfaces in reflection. wgslpp reads these via naga's doc-comment AST, so the same `///` text is a real WGSL doc comment — no preprocessing required.

| Marker                | Applies to                             | Effect on reflection   |
| --------------------- | -------------------------------------- | ---------------------- |
| `/// @unfilterable`   | textures                               | `unfilterable: true`   |
| `/// @nonfiltering`   | samplers (not `sampler_comparison`)    | `nonfiltering: true`   |
| `/// @dynamic_offset` | uniform / storage / storage_rw buffers | `dynamic_offset: true` |

```wgsl
/// @unfilterable
@group(0) @binding(0) var depth_image: texture_2d<f32>;

/// @nonfiltering
@group(0) @binding(1) var depth_sampler: sampler;

/// @dynamic_offset
@group(1) @binding(0) var<uniform> renderable: PerRenderableData;
```

The marker body must match exactly (after stripping the `///` and surrounding whitespace) — a `///` comment with extra prose like `/// renderable transform buffer` is treated as documentation and ignored.

Multi-sampled textures (`texture_multisampled_*`) are intrinsically unfilterable per WebGPU spec and get `unfilterable: true` automatically — no marker needed.

## Reflection Output

`wgslpp reflect` outputs JSON describing the shader interface:

```json
{
  "bindings": [
    {
      "group": 0,
      "binding": 0,
      "name": "frameUniforms",
      "type": "uniform",
      "wgsl_type": "FrameUniforms"
    },
    {
      "group": 0,
      "binding": 10,
      "name": "depth_image",
      "type": "texture",
      "wgsl_type": "texture_2d<f32>",
      "unfilterable": true
    },
    {
      "group": 0,
      "binding": 11,
      "name": "depth_sampler",
      "type": "sampler",
      "wgsl_type": "sampler",
      "nonfiltering": true
    },
    {
      "group": 1,
      "binding": 0,
      "name": "renderable",
      "type": "uniform",
      "wgsl_type": "PerRenderableData",
      "dynamic_offset": true
    }
  ],
  "structs": [
    {
      "name": "MaterialParams",
      "size": 48,
      "alignment": 16,
      "fields": [
        { "name": "color", "type": "vec3<f32>", "offset": 0, "size": 12 },
        { "name": "roughness", "type": "f32", "offset": 12, "size": 4 }
      ]
    }
  ],
  "entry_points": [
    { "name": "vs_main", "stage": "vertex", "workgroup_size": null },
    { "name": "fs_main", "stage": "fragment", "workgroup_size": null }
  ]
}
```

**`type`** — category: `uniform`, `storage_read`, `storage_read_write`, `sampler`, `sampler_comparison`, `texture`.

**`wgsl_type`** — full WGSL type for `BindGroupLayoutEntry` generation:

| Category | `wgsl_type` values                                                                                                                                                                                                                                                       |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Buffers  | The struct name (e.g. `FrameUniforms`, `PerRenderableData`)                                                                                                                                                                                                              |
| Textures | `texture_2d<f32>`, `texture_2d<i32>`, `texture_2d<u32>`, `texture_cube<f32>`, `texture_2d_array<f32>`, `texture_3d<f32>`, `texture_multisampled_2d<f32>`, `texture_depth_2d`, `texture_depth_2d_array`, `texture_depth_cube`, `texture_storage_2d<FORMAT, ACCESS>`, etc. |
| Samplers | `sampler`, `sampler_comparison`                                                                                                                                                                                                                                          |

Marker flags (`unfilterable`, `nonfiltering`, `dynamic_offset`) appear on the matching binding kinds; see [Binding Marker Comments](#binding-marker-comments).

Stages: `vertex`, `fragment`, `compute`, `task`, `mesh`.

## Optimization

### Dead Code Elimination

`--dce` walks from all entry points (`@vertex`, `@fragment`, `@compute`) and transitively marks reachable functions. Unreachable functions are removed from the module before output.

### Identifier Renaming

`--rename` assigns the shortest available names to the most frequently used identifiers. Entry point names and `@group/@binding`-decorated globals are preserved (they're visible to the host API). Short names follow the sequence: `a`–`z`, `A`–`Z`, `aa`, `ab`, etc. WGSL keywords are avoided.

## LSP Server

Start the language server:

```sh
wgslpp-lsp
```

Communicates over stdio. Supports:

| Feature                   | Details                                                                                                                                   |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| **Diagnostics**           | Live validation on every edit via naga, with source-map remapping to original files                                                       |
| **Go-to-definition**      | `#include` path navigation, symbol lookup across the include graph (parameters, struct fields, locals)                                    |
| **Hover**                 | Function signatures, struct layouts with byte offsets, binding info, parameters and locals with their types                               |
| **Completion**            | WGSL keywords, 50+ builtin functions with signatures, user symbols, `#include` paths, preprocessor directives (triggers: `.` `#` `<` `"`) |
| **Document symbols**      | Functions, structs, globals, constants, entry points (nested)                                                                             |
| **Semantic highlighting** | Preprocessor directives, attributes, `#include` paths                                                                                     |
| **Folding ranges**        | `#ifdef`/`#endif` blocks, `{}` blocks                                                                                                     |
| **Formatting**            | naga writer-based, configurable indent                                                                                                    |

### Configuration

Place `wgslpp.json` at the workspace root:

```json
{
  "packages": [
    { "name": "polymer", "path": "./polymer/shaders" },
    { "name": "common", "path": "./shared/shaders" }
  ],
  "configurations": {
    "unlit": { "defines": { "SHADING_MODEL_UNLIT": "" } },
    "pbr": { "defines": { "SHADING_MODEL_PBR": "" } }
  }
}
```

## Editor Plugins

### VS Code

Located in `wgslpp-vscode/`. Provides:

- LSP client (diagnostics, completion, hover, go-to-definition, formatting)
- TextMate grammar for syntax highlighting (WGSL + preprocessor directives)
- 10 code snippets (`@vertex`, `@fragment`, `@compute`, `fn`, `struct`, `uniform`, `storage`, `texture`, `#ifdef`, `#include`)
- Commands: Validate Current File, Show Preprocessed Output, Restart Language Server

The published `.vsix` bundles the matching `wgslpp-lsp` binary, so users don't need a separate install. For local development, either set `wgslpp.binary.path` or build from source — the extension also looks under `target/release/` and `target/debug/`.

**Settings:**

| Setting                      | Default | Description                                       |
| ---------------------------- | ------- | ------------------------------------------------- |
| `wgslpp.binary.path`         | `""`    | Path to `wgslpp-lsp` binary (overrides discovery) |
| `wgslpp.includePaths`        | `[]`    | Additional include search paths                   |
| `wgslpp.validateOnSave`      | `true`  | Validate on save                                  |
| `wgslpp.validateOnType`      | `true`  | Validate as you type                              |
| `wgslpp.diagnostics.enabled` | `true`  | Enable diagnostics                                |
| `wgslpp.inlayHints.enabled`  | `false` | Enable inlay hints                                |
| `wgslpp.format.indentSize`   | `4`     | Formatting indent size                            |

### CLion / IntelliJ

Located in `wgslpp-clion/`. Build with:

```sh
cd wgslpp-clion && ./gradlew buildPlugin
```

Install the resulting `build/distributions/wgslpp-clion-0.1.0.zip` via **Settings → Plugins → Install Plugin from Disk**.

Provides:

- LSP-powered diagnostics, completion, hover, go-to-definition, formatting
- TextMate grammar for syntax highlighting (shared with VS Code)
- 10 live templates (same snippets as VS Code)
- Settings UI at **Settings → Tools → WGSL++** for the binary path

Requires CLion/IntelliJ 2025.1 or later.

## Project Structure

```
wgslpp/
  crates/
    wgslpp-preprocess/    # Preprocessor: #include, #define, #ifdef, #pragma once, source maps
    wgslpp-core/          # Validation (naga), reflection, minification, DCE, renaming
    wgslpp-cli/           # CLI binary (clap): preprocess, validate, reflect, minify, pipeline
    wgslpp-lsp/           # LSP server: diagnostics, navigation, hover, completion, formatting
  npm/
    cli/                  # @polycam/wgslpp — JS launcher + typed Node API
    cli-darwin-arm64/     # @polycam/wgslpp-darwin-arm64 (binary filled at release)
    cli-linux-x64/        # @polycam/wgslpp-linux-x64
    cli-win32-x64/        # @polycam/wgslpp-win32-x64
  wgslpp-vscode/          # VS Code extension
  wgslpp-clion/           # CLion/IntelliJ plugin
  tests/
    integration/          # Integration tests
    external/miniray/     # Imported miniray test suite (CC0)
    preprocess/           # Preprocessor test fixtures
  .github/workflows/      # CI + release automation
```

## Dependencies

| Crate                  | Version | Purpose                                   |
| ---------------------- | ------- | ----------------------------------------- |
| `naga`                 | 28      | WGSL parsing, validation, code generation |
| `clap`                 | 4       | CLI argument parsing                      |
| `lsp-server`           | 0.7     | LSP protocol transport                    |
| `lsp-types`            | 0.97    | LSP type definitions                      |
| `serde` / `serde_json` | 1       | Serialization                             |
| `thiserror`            | 2       | Error types                               |

## Testing

```sh
cargo test
```

128 tests across the workspace:

- 49 preprocessor unit tests (conditionals, macros, includes, `#pragma once`, expression evaluator, UTF-8 safety, comparison operators, recursive define resolution)
- 24 core unit tests (DCE, renaming, short name generation, keyword safety, reflection `wgsl_type` for all binding kinds, doc-comment marker extraction)
- 11 LSP unit + e2e tests (cross-include navigation, parameter/field detection, attributed entry points)
- 44 integration tests (10 pipeline + 34 imported miniray suite covering validation, DCE, sample roundtripping, reflection preservation)

## Releasing

Releases produce three deliverables, all from one tag push:

1. **npm packages** — `@polycam/wgslpp` (meta) + three platform packages (`@polycam/wgslpp-{darwin-arm64,linux-x64,win32-x64}`).
2. **VS Code extension** — three platform-specific `.vsix` files attached to the GitHub Release. Marketplace publishing is opt-in (uncomment the `vsce publish` step in `.github/workflows/release.yml`).
3. **GitHub Release** — raw `.tar.gz` of each platform's binaries plus the `.vsix` files, for users who don't want npm.

### Cutting a release

1. **Bump the workspace version**. `crates/wgslpp-cli/Cargo.toml` is the source of truth — keep the other crate versions in sync if you care.

   ```sh
   cargo set-version -p wgslpp-cli 0.2.0
   cargo set-version -p wgslpp-lsp 0.2.0
   cargo set-version -p wgslpp-core 0.2.0
   cargo set-version -p wgslpp-preprocess 0.2.0
   cargo build --workspace  # refresh Cargo.lock
   ```

2. **Commit & tag**:

   ```sh
   git commit -am "Release 0.2.0"
   git tag v0.2.0
   git push origin main --tags
   ```

3. **Watch CI**. The `Release` workflow runs:
   - `check-version` — fails fast unless the tag matches `crates/wgslpp-cli/Cargo.toml`. Prevents publishing artifacts that disagree with `wgslpp --version`.
   - `build` — fans out to ubuntu-latest / macos-14 / windows-latest, each producing the `wgslpp` + `wgslpp-lsp` binaries for its native target.
   - `npm` — stages the three platform binaries into `npm/cli-*/bin/`, stamps the version into all four `package.json` files, publishes platform packages first, then the meta package. Uses npm trusted publishing (OIDC) — no `NPM_TOKEN` needed.
   - `vscode` — packages a `.vsix` per platform with the matching `wgslpp-lsp` binary inside `wgslpp-vscode/bin/`.
   - `release` — creates the GitHub Release and uploads the raw archives plus the three `.vsix` files.

### Required secrets

- `VSCE_PAT` _(only if you uncomment marketplace publishing)_ — Azure DevOps PAT scoped to the VS Code marketplace publisher.

`GITHUB_TOKEN` is provided by Actions automatically. npm publishing uses **trusted publishing** (OIDC) — the workflow exchanges a short-lived GitHub-issued OIDC token for a publish credential at request time, so there's no long-lived `NPM_TOKEN` in repo secrets. Each of the four packages must have this repo + `release.yml` configured as a trusted publisher on npmjs.com (Settings → Publishing access).

### Where versions live

| Place                        | Set by                 | Used for                            |
| ---------------------------- | ---------------------- | ----------------------------------- |
| `crates/*/Cargo.toml`        | You, before tagging    | `wgslpp --version`, `cargo install` |
| `vX.Y.Z` git tag             | You, after committing  | Release trigger + identity          |
| `npm/*/package.json`         | CI, at publish time    | npm registry. `0.0.0` at rest       |
| `wgslpp-vscode/package.json` | CI, at vsix-build time | VS Code marketplace                 |

The npm `package.json` files are intentionally pinned to `0.0.0` in the repo — CI overwrites them per-release. Don't hand-edit them; that won't change what gets published. The VS Code extension's `package.json` is left at the current Cargo version so local "Run Extension" debugging shows a meaningful version, but CI overwrites it before packaging the released `.vsix`.

## Acknowledgments

- **[naga](https://github.com/gfx-rs/wgpu/tree/trunk/naga)** (gfx-rs) — the WGSL frontend, validator, and writer that wgslpp's `validate`, `reflect`, and `minify` are built on. The reflection schema mirrors naga's IR, doc-comment markers ride on naga's parser, and the WGSL output of `--minify` is naga's writer.
- **[miniray](https://github.com/h3r2tic/miniray)** (Tomasz Stachowiak) — the seed for wgslpp's minification pipeline. The dead-code elimination, identifier renaming, and short-name allocation logic in `wgslpp-core` is a Rust port of miniray's WGSL post-processing, and the test suite under `tests/external/miniray/` (65 sample shaders, validation fixtures, snapshot baselines) is imported under CC0.

## License

MIT

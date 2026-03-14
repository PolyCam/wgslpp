# wgslpp

A Rust toolchain for WGSL shaders: `#include`/`#define`/`#pragma once` preprocessing, naga-based validation, dead code elimination, identifier renaming, reflection, and a full LSP server.

## Overview

wgslpp consolidates WGSL shader preprocessing, validation, optimization, and editor tooling into a single binary. The CLI exposes individual stages or an all-in-one `pipeline` subcommand for build integration. The LSP server provides live diagnostics, go-to-definition, hover, completion, semantic highlighting, formatting, and more. Editor plugins are included for both VS Code and CLion/IntelliJ.

## Building

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

| Flag | Description |
|------|-------------|
| `-P <name=path>` | Named package mapping (repeatable). Maps `#include <name/...>` to a directory. |
| `-D <name[=value]>` | Define a preprocessor macro (repeatable) |
| `-o, --output <path>` | Output file (default: stdout) |
| `--source-map <path>` | Write source map JSON |

### `wgslpp validate`

Parse and validate WGSL using naga.

```
wgslpp validate <input> [--source-map file] [--format human|json|gcc]
```

| Flag | Description |
|------|-------------|
| `--source-map <path>` | Remap error locations through a source map |
| `--format <format>` | Diagnostic output format: `human` (default), `json`, `gcc` |

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

| Flag | Description |
|------|-------------|
| `--dce` | Remove unreachable functions (dead code elimination) |
| `--rename` | Frequency-based identifier shortening |

### `wgslpp pipeline`

All-in-one: preprocess → validate → reflect → minify.

```
wgslpp pipeline <input> [-P <name=path>]... [-D <name[=val]>]... [-o output] \
    [--reflect file.json] [--source-map file] [--no-validate] [--no-minify] [--dce] [--rename]
```

This is the primary build integration point — a single invocation replaces separate preprocessing, validation, reflection, and optimization steps.

## Preprocessor Directives

| Directive | Description |
|-----------|-------------|
| `#include "path.wgsl"` | Textual inclusion, relative to the current file |
| `#include <pkg/path.wgsl>` | Package-scoped inclusion (resolved via `-P` flag or `wgslpp.json`) |
| `#pragma once` | Include this file at most once |
| `#define NAME` | Define a flag |
| `#define NAME value` | Object-like macro (text replacement) |
| `#define NAME(a, b) body` | Function-like macro with parameter substitution |
| `#undef NAME` | Remove a definition |
| `#ifdef NAME` / `#ifndef NAME` | Conditional on definition existence |
| `#if EXPR` / `#elif EXPR` | Conditional on expression value |
| `#else` / `#endif` | Conditional block control |

### Expression Grammar

`#if` and `#elif` support a C-like expression language:

```
expr       = or_expr
or_expr    = and_expr ("||" and_expr)*
and_expr   = bitor_expr ("&&" bitor_expr)*
bitor_expr = cmp_expr ("|" cmp_expr)*
cmp_expr   = bitand_expr (("==" | "!=") bitand_expr)?
bitand_expr = unary_expr ("&" unary_expr)*
unary_expr = "!" unary_expr | primary
primary    = "defined" "(" IDENT ")" | "(" expr ")" | NUMBER | IDENT
```

### Package-Scoped Includes

Register named packages via `-P` on the CLI or in `wgslpp.json`:

```sh
wgslpp pipeline main.wgsl -P polymer=./polymer/shaders -P common=./shared/shaders
```

Then in WGSL:

```wgsl
#include <polymer/lighting/pbr.wgsl>
#include <common/math.wgsl>
```

`#include <polymer/lighting/pbr.wgsl>` resolves to `./polymer/shaders/lighting/pbr.wgsl`.

## Reflection Output

`wgslpp reflect` outputs JSON describing the shader interface:

```json
{
  "bindings": [
    { "group": 0, "binding": 0, "name": "uniforms", "type": "uniform" }
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

Binding types: `uniform`, `storage`, `storage_rw`, `sampler`, `texture`, `handle`.
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

| Feature | Details |
|---------|---------|
| **Diagnostics** | Live validation on every edit via naga, with source-map remapping to original files |
| **Go-to-definition** | `#include` path navigation, text-based symbol lookup across files |
| **Hover** | Function signatures, struct layouts with byte offsets, binding info, constant types |
| **Completion** | WGSL keywords, 50+ builtin functions with signatures, user symbols, `#include` paths, preprocessor directives (triggers: `.` `#` `<` `"`) |
| **Document symbols** | Functions, structs, globals, constants, entry points (nested) |
| **Semantic highlighting** | Preprocessor directives, attributes, `#include` paths |
| **Folding ranges** | `#ifdef`/`#endif` blocks, `{}` blocks |
| **Formatting** | naga writer-based, configurable indent |

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
    "pbr":   { "defines": { "SHADING_MODEL_PBR": "" } }
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

**Settings:**

| Setting | Default | Description |
|---------|---------|-------------|
| `wgslpp.binary.path` | `""` | Path to `wgslpp-lsp` binary (searches PATH if empty) |
| `wgslpp.includePaths` | `[]` | Additional include search paths |
| `wgslpp.validateOnSave` | `true` | Validate on save |
| `wgslpp.validateOnType` | `true` | Validate as you type |
| `wgslpp.diagnostics.enabled` | `true` | Enable diagnostics |
| `wgslpp.inlayHints.enabled` | `false` | Enable inlay hints |
| `wgslpp.format.indentSize` | `4` | Formatting indent size |

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
  wgslpp-vscode/          # VS Code extension
  wgslpp-clion/           # CLion/IntelliJ plugin
  tests/
    integration/          # Integration tests
    external/miniray/     # Imported miniray test suite (CC0)
    preprocess/           # Preprocessor test fixtures
```

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `naga` | 28 | WGSL parsing, validation, code generation |
| `clap` | 4 | CLI argument parsing |
| `lsp-server` | 0.7 | LSP protocol transport |
| `lsp-types` | 0.97 | LSP type definitions |
| `serde` / `serde_json` | 1 | Serialization |
| `thiserror` | 2 | Error types |

## Testing

```sh
cargo test
```

86 tests across the workspace:
- 35 preprocessor unit tests (conditionals, macros, includes, `#pragma once`, expression evaluator)
- 7 core unit tests (DCE, renaming, short name generation, keyword safety)
- 10 integration pipeline tests
- 34 imported miniray tests (validation of 65 WGSL files, DCE behavior, sample roundtripping, reflection preservation)

## License

MIT

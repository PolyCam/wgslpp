# WGSL++

WGSL language support for VS Code, backed by the [`wgslpp`](https://github.com/PolyCam/wgslpp) toolchain.

WGSL on its own has no `#include`, no `#define`, no preprocessor. This extension wraps `wgslpp`'s preprocessor, naga-based validator, and reflection so they all run live as you edit.

## Features

- **Diagnostics** — live validation on every keystroke, with errors remapped through `#include` source maps so messages point at the original file and line, not the inlined output.
- **Go-to-definition** — works across `#include`d files, into struct fields and function parameters.
- **Hover** — function signatures, struct layouts with byte offsets, binding info, parameters and locals with their resolved types.
- **Completion** — WGSL keywords, 50+ builtin functions with signatures, user symbols, `#include` paths, preprocessor directives.
- **Document symbols** — functions, structs, globals, constants, entry points (nested).
- **Semantic highlighting + folding** — preprocessor directives, attributes, `#include` paths; `#ifdef`/`#endif` blocks fold cleanly.
- **Formatting** — naga's WGSL writer normalises whitespace and re-indents.
- **Snippets** — `@vertex`, `@fragment`, `@compute`, `fn`, `struct`, `uniform`, `storage`, `texture`, `#ifdef`, `#include`.
- **TextMate grammar** — syntax highlighting for WGSL plus the preprocessor directives.

## Preprocessor support

`#include "..."` (relative) and `#include <pkg/...>` (resolved via `wgslpp.json`), `#define` (object + function-like), `#ifdef` / `#ifndef` / `#if` / `#elif` / `#else` / `#endif` with a C-like expression grammar (decimal + hex literals, comparisons, recursive identifier resolution), `#pragma once`, `#undef`. See the [main repo README](https://github.com/PolyCam/wgslpp#preprocessor-directives) for the full grammar.

## Marker comments

Tag textures and samplers with doc-comment markers that surface in reflection:

```wgsl
/// @unfilterable
@group(0) @binding(0) var depth_image: texture_2d<f32>;

/// @nonfiltering
@group(0) @binding(1) var depth_sampler: sampler;

/// @dynamic_offset
@group(1) @binding(0) var<uniform> renderable: PerRenderableData;
```

## Setup

### Workspace config

Create a `wgslpp.json` at the workspace root for package-style includes and conditional compilation:

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

### Bundled language server

The extension ships with a platform-specific `wgslpp-lsp` binary — no separate install. Override the path with `wgslpp.binary.path` if you've built your own.

## Settings

| Setting | Default | Description |
|---|---|---|
| `wgslpp.binary.path` | `""` | Path to `wgslpp-lsp` binary (overrides bundled discovery) |
| `wgslpp.includePaths` | `[]` | Additional include search paths |
| `wgslpp.validateOnSave` | `true` | Validate on save |
| `wgslpp.validateOnType` | `true` | Validate as you type |
| `wgslpp.diagnostics.enabled` | `true` | Enable diagnostics |
| `wgslpp.format.indentSize` | `4` | Formatting indent size |

## Commands

- **WGSL++: Validate Current File**
- **WGSL++: Show Preprocessed Output**
- **WGSL++: Restart Language Server**

## CLI counterpart

The CLI is published separately as [`@polycam/wgslpp`](https://www.npmjs.com/package/@polycam/wgslpp) on npm — same toolchain, scriptable for build systems.

## License

[MIT](LICENSE)

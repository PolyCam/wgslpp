# @polycam/wgslpp

WGSL preprocessor, validator, reflector, and minifier — distributed as a
prebuilt native binary plus a typed Node API.

## Install

```sh
npm install -g @polycam/wgslpp
# or, per-project
npm install --save-dev @polycam/wgslpp
```

The right binary for your platform is pulled in automatically via
`optionalDependencies`. Supported platforms:

- macOS arm64 (Apple Silicon)
- Linux x86_64
- Windows x86_64

## CLI

```sh
wgslpp --help
wgslpp preprocess input.wgsl -o out.wgsl
wgslpp validate input.wgsl
wgslpp reflect input.wgsl
wgslpp pipeline --input input.wgsl --config wgslpp.json
```

## API

Every CLI subcommand has a matching exported function — sync and async,
sharing the same options shape. `pipeline()` is the one most build
scripts want (preprocess + validate + reflect in a single subprocess);
the others exist for completeness.

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

// All-in-one: preprocess + validate + reflect (+ optional minify).
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

// Subcommand wrappers for one-shot usage.
const pp = preprocess({ input: "x.wgsl" }); // { code }
const v = validate({ input: "x.wgsl" }); // { valid, diagnostics }
const r = reflect({ input: "x.wgsl" }); // ReflectionData
const m = minify({ input: "x.wgsl", dce: true }); // { code }
```

`validate()` never throws on validation errors — it returns
`{ valid: false, diagnostics }` so callers decide how to react.
Subprocess-level failures (binary missing, output unparseable) throw a
`WgslppError` with `stderr` and `exitCode` attached.

`BindingInfo` is a discriminated union over `BufferBinding`,
`SamplerBinding`, and `TextureBinding`. Each kind only exposes the marker
flags that apply to it — there's no way to ask a sampler if it's
`unfilterable`, because that question doesn't make sense.

The `*Async` variants return Promises; otherwise identical.

### Binary discovery

Resolution order, used by both the CLI launcher and the API:

1. The `binPath` option on any subcommand (API only).
2. The `WGSLPP_BIN` env var.
3. The platform sub-package matching `process.platform`/`process.arch`.

`resolveBinary()` is exported for callers that want to spawn the binary
themselves.

## Why a JS shim

`@polycam/wgslpp` is a tiny launcher and a typed wrapper. The actual
binary ships in a platform-specific sub-package
(`@polycam/wgslpp-<platform>-<arch>`). npm drops the sub-packages whose
`os`/`cpu` constraints don't match the host, so each install gets exactly
one binary and no cross-compile noise.

If installation skipped optional deps (some CI configurations do), re-run
with `--include=optional`.

## Source

<https://github.com/PolyCam/wgslpp>

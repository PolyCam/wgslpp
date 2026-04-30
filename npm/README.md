# npm packaging

This directory holds the npm package layout for distributing `wgslpp` as a
prebuilt binary on the npm registry.

## Layout

```
npm/
  cli/                    @polycam/wgslpp — TS API + CLI launcher
    src/                  TypeScript source (typed API + binary resolver)
    dist/                 compiled JS + .d.ts (built by CI, gitignored)
    tsconfig.json
    package.json
  cli-darwin-arm64/       @polycam/wgslpp-darwin-arm64
    bin/wgslpp            (filled by CI)
  cli-linux-x64/          @polycam/wgslpp-linux-x64
    bin/wgslpp            (filled by CI)
  cli-win32-x64/          @polycam/wgslpp-win32-x64
    bin/wgslpp.exe        (filled by CI)
```

The meta package declares the three platform packages as
`optionalDependencies` with `os`/`cpu` constraints. npm will install only
the one that matches the host. The launcher (`bin/wgslpp.js`) resolves the
matching package via Node's module resolution and execs its binary with the
user's argv.

## Releasing

This is wired into `.github/workflows/release.yml`. The flow is:

1. Bump the workspace version in `crates/wgslpp-cli/Cargo.toml` (and any
   other crate versions you want kept in sync), commit, push.
2. `git tag vX.Y.Z && git push --tags`.
3. CI builds the three platform binaries, copies each one into the matching
   `npm/cli-*/bin/` dir, stamps the version into all four `package.json`
   files, then publishes them to npm with `NPM_TOKEN`.

The version inside this directory is `0.0.0` at rest — CI overwrites it at
publish time. Do not hand-edit the npm versions here; they should always
match the Cargo.toml version of the corresponding release.

## Local smoke test

To verify the launcher and API resolve correctly without publishing:

```sh
cargo build --release -p wgslpp-cli
( cd npm/cli && npm install && npm run build )

# Pack both, install into a scratch project.
( cd npm/cli && npm pack )
cp target/release/wgslpp npm/cli-darwin-arm64/bin/wgslpp
( cd npm/cli-darwin-arm64 && npm pack )

mkdir /tmp/wgslpp-smoke && cd /tmp/wgslpp-smoke
npm init -y >/dev/null
npm install /Users/.../npm/cli/polycam-wgslpp-0.0.0.tgz \
            /Users/.../npm/cli-darwin-arm64/polycam-wgslpp-darwin-arm64-0.0.0.tgz
node -e 'import("@polycam/wgslpp").then(m => console.log(m.resolveBinary()))'
npx wgslpp --version
```

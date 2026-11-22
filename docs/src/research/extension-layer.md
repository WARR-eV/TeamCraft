# A sandboxed extension layer

*2026-11-22* – issue #382

## Question

`docs/OPEN_WORK.md` asks for *a plugin/extension layer* and *low-code
validation rules*: organisations want their own numbering, naming and
release checks – later their own file readers or exports – without forking
TeamCraft. How can TeamCraft run code it did not write, inside the server,
without it reading other groups' data, reaching the network or taking the
server down?

## Options

| Option | For | Against |
|--------|-----|---------|
| Scripts in an embedded language (Lua, Rhai, JavaScript engine) | quick to write | each language is its own sandbox to audit; limits on memory and time are engine-specific; JavaScript engines are large C/C++ code bases (against "pure Rust") |
| Out-of-process hooks (webhooks that answer) | any language, isolation by the OS | network round trip in a transaction, availability couples to an outside service; TeamCraft already has outgoing webhooks for notifications, not for decisions |
| Dynamic libraries | fast | no sandbox at all, ABI breaks |
| **WebAssembly components** (WASI-free, own WIT interface) | one sandbox for every source language; memory, fuel (instructions) and wall-time limits built in; imports are exactly what the host links – nothing else exists for the guest; a typed, versioned interface (WIT) | a runtime dependency; plugin authors need a WebAssembly toolchain |

Runtimes in pure Rust: **wasmtime** (Bytecode Alliance, Apache-2.0 WITH
LLVM-exception; component model, fuel, epoch interruption, memory limits via
`ResourceLimiter`) – the reference implementation of the component model;
**wasmi** (MIT/Apache-2.0, an interpreter; component model not yet
complete). wasmtime is large but mature and audited (it backs production
FaaS platforms); its JIT (Cranelift) is pure Rust.

Prior art in the open-source CAD neighbourhood: SolidCraft's plugin runtime
(MIT/Apache-2.0) uses exactly this shape – components against a versioned
WIT package, capabilities as `name@major.minor`, approval by a SHA-256 over
manifest and module, fuel/memory/deadline per call, no network imports,
one transaction per call. TeamCraft's own plugin *for* SolidCraft
(`plugins/solidcraft`) is built against it. The design below follows the
same principles; it is written for TeamCraft's data, not copied.

## Decision

**WebAssembly components on wasmtime**, behind a cargo feature
(`extensions`, off in the first releases), with these rules:

1. **Packages** – a folder with `extension.json` (`id`, `name`, `version`,
   `description`, `capabilities`, entry points, limits) and one component.
   Admins install them (*Admin → Extensions*, or a mounted folder); installing
   or enabling runs nothing.
2. **Approval by content** – *Inspect* reads manifest and component without
   running them, shows the capabilities, entry points, limits and a SHA-256
   over both files. An admin approves that hash; a changed file needs a new
   approval. Approvals, enable/disable and every refused call are audited.
3. **Capabilities, versioned** – a package needing an unknown or newer
   capability is refused in plain words ("needs validate 2; update
   TeamCraft"). First ones:
   * `validate@1` – called before a lifecycle transition (submit for review,
     release) with the revision, its attributes, files (metadata, not
     content) and BOM lines; returns findings (`error` blocks, `warning`
     shows). The *low-code validation rules* of OPEN_WORK become a built-in
     extension of this kind.
   * `numbering@1` – proposes a number for a new item from the group, item
     type and attributes; the numbering scheme still checks uniqueness.
   * later `files.read@1` – reads one file of the revision being checked
     (bounded, through the vault), for content checks.
4. **Sandbox** – no WASI, no network, no clock (a fixed time unless granted),
   no file system. Per call: fuel limit, memory limit (default 64 MiB),
   wall-time limit (epoch interruption). A trap or timeout fails the call
   with a clear message and never the server; three failures in a row
   disable the extension and notify admins.
5. **Data access** – only through host functions that run with the acting
   user's rights (`Actor`), so an extension never sees what the user may not
   see. Results are findings or proposals; any write still goes through
   `services::*` and the audit log.
6. **Where it runs** – synchronously inside the transition's transaction for
   `validate` (bounded by the limits), never on the request path otherwise.

## Threat model additions

Malicious or buggy extension code: contained by the sandbox (no imports
beyond the host's), resource limits, rights of the acting user, approval by
hash, audit. A compromised admin account can approve anything – the same
as today with configuration. Denial of service through slow extensions:
fuel and wall-time limits, automatic disable.

## Next

Implementation issues: (1) runtime + manifest + approval + admin page
behind the feature flag; (2) `validate@1` at lifecycle transitions with an
example extension in `examples/extensions/`; (3) `numbering@1`.

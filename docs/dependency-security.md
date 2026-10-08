# Dependency management and supply-chain security

Status: the initial pure-source manager is implemented in Phase 22. See
[the executable workflow and current schemas](packages.md) and
[ADR 0051](adr/0051-verified-source-packages.md). The requirements below remain
the long-term security contract. Public registry infrastructure, provenance,
authorized build execution and sandboxing are not implemented.

## Principles and threat model

“Dependency resolution should be boring, deterministic and auditable.”

“Dependencies are data until explicitly granted execution authority.”

“Tarn should assume that any dependency, including a transitive dependency, may become hostile.”

The threat model includes malicious or compromised direct and transitive
packages, compromised publisher accounts, malicious new releases, typosquatting
and dependency confusion. Official status, popularity, a familiar publisher and
previously safe releases do not establish code trust. Publisher authentication
establishes identity, not safety. Language memory safety does not make hostile
code harmless or replace supply-chain controls.

## One CLI, version-free imports

Package operations belong to the existing `tarn` CLI, never a separate `tarnpkg`.
The intended ordinary workflow is:

```sh
tarn add postgres
tarn add redis
tarn build
tarn test
```

These commands require actual releases in a configured registry; no public postgres or redis packages are assumed.
Source imports remain `import "redis"` and `import "postgres"`; versions must
not leak into paths such as `redis/v2` or `postgres@4`. Local filesystem module
semantics remain intact. The initial manager maps declared dependencies to isolated module namespaces and rejects local/package ambiguity.

`tarn.toml` records dependency intent and security policy; `tarn.lock` records
the exact resolved graph. Intent must express exact versions, compatible-major
requirements, compatible minor/patch requirements and explicit ranges using
ordinary SemVer, with understandable conflict explanations. The initial manifest and bounded one-version resolution algorithm are specified in ADR 0051; incompatible-major coexistence remains open.

Prefer a single compatible version where constraints allow it; avoid unnecessary
duplicates. Multiple incompatible majors may coexist only under an explicit
future resolver design, never by requiring versioned source imports.

## Locking, identity and verification

Normal builds must never silently rewrite the lockfile. Intentional changes use
explicit operations such as `tarn update` or `tarn update <package>`; add/remove
must make their corresponding graph changes reviewable. Missing or incompatible
locks require explicit resolution rather than a hidden build-time update.

Stable invariant: the same source tree, compiler/toolchain identity and lockfile
resolve to the same dependency bytes and graph. This does not promise bit-identical
native outputs, which also depend on build inputs and toolchain behavior, and is
not a claim about the current compiler's package support.

At minimum, each locked package records its identity, exact version, source
registry and content hash, plus graph edges. Bind identities to explicit origins
so a same-named package from another registry cannot silently substitute for it.
Verify content before use, including cached sources. A hash mismatch fails;
refetching cannot authorize replacement bytes under the same locked identity.

Future lock metadata should carry publisher, repository, commit, publication
mechanism, signatures and provenance evidence. Content integrity, identity and
publication evidence are distinct from trusting package behavior. Evidence
formats and verification mechanisms are not yet selected.

A registry release is immutable: a published package identity/version/content
hash cannot later mean different bytes. Yanking or deprecation can change
availability or advisory status, not content. Changed bytes require a new version.
Policies for consuming already locked yanked releases still need design.

## Packages do not execute on installation

Fetching, resolving or installing packages grants no execution authority by
default. There are no automatic preinstall, install, postinstall, prepare,
setup.py or build.rs equivalents. Pure Tarn packages normally need no such step.
Compiling their source is distinct from executing package-supplied build code.

If native integration or another justified use requires build execution, it must
be explicitly enabled, permissioned and sandboxed. Declaration alone is not a
permission grant. Capability requests must include filesystem, network, process
execution and environment access. An illustrative, non-final schema is:

```toml
[build.permissions]
filesystem = "package"
network = false
environment = ["CC"]
process = ["cc"]
```

The future implementation must define read/write roots, executable identity,
child-process authority and enforcement. There is no ambient access to `$HOME`,
`.ssh`, cloud or package credentials, arbitrary environment variables, network or
the entire filesystem. Such access requires explicit authority; it cannot be
inherited merely because the CLI has it. Required containment must fail closed
when unavailable, rather than run unsandboxed for convenience.

The Rust/Cargo dependencies used to build today's compiler remain governed by
its existing dependency review policy. The compiler currently invokes system
`cc` for its embedded C runtime; this is an existing trusted toolchain boundary,
not permission for arbitrary package scripts. No sandbox is claimed for today's
compiler or Cargo builds. Future package-provided native build steps must obey
the capability model even if they also invoke `cc`.

## Content-addressed storage

The conceptual global store is:

```text
~/.tarn/registry/    registry metadata and evidence
~/.tarn/sources/     verified sources addressed by content
~/.tarn/artifacts/   reusable build artifacts
```

The exact disk layout remains open. Deduplicate source bytes by content hash,
while retaining origin and trust metadata independently. Artifact identity must
include source hash, compiler/toolchain identity, target, build options and the
resolved dependency graph identity. Package names, mutable tags or dependency
interface hashes alone are insufficient. Build execution inputs and granted
capabilities must also be accounted for when deciding artifact reuse; cache hits
cannot bypass verification or project security policy. This is a storage design
requirement, not an optimization implementation.

## Release age and trust changes

Support configurable minimum release age at project and global scope. For
example, conceptually:

```toml
[security]
minimum_release_age = "72h"
```

Seventy-two hours is illustrative, not the chosen default. The default, policy
precedence, authenticated release-time evidence and exception handling are open.
Age reduces immediate exposure; it does not certify safety.

Detect trust downgrades: missing previously available provenance, publisher
changes, unverified publication mechanisms and unexpected repository changes.
A SemVer-compatible update does not authorize these changes. Show the delta and
require explicit user acknowledgement; required policy checks may still refuse
the release. Record enough evidence to audit what changed and what was accepted.
Insufficient required trust/provenance, content mismatches or missing permissions
must produce explicit refusal. Never silently use a supposedly trusted fallback
for convenience.

## Inspection and registry requirements

The command surface is implemented for the bootstrap subset; the full trust responsibilities below remain requirements:

| Command | Intended responsibility |
|---------|-------------------------|
| `tarn add <package>` | Declare intent and explicitly resolve the resulting graph |
| `tarn remove <package>` | Remove intent and explicitly update the graph |
| `tarn update [package]` | Intentionally update all or selected dependencies |
| `tarn deps` / `tarn deps --tree` | Inspect exact versions and dependency paths |
| `tarn deps --why <package>` | Explain which dependency introduced a package and why it resolved |
| `tarn deps --trust` | Inspect origins, publisher/provenance changes and execution authority |
| `tarn audit` | Report applicable security advisories and policy findings |
| `tarn verify` | Verify locked content and required publication evidence |
| `tarn publish` | Publish an immutable release with required identity/evidence checks |

Inspection must expose version, origin, hash, dependency introducer, publisher,
provenance changes, requested/granted build permissions and security advisories.
Distinguish unavailable evidence from successful verification; audit success does
not prove absence of vulnerabilities. Keep routine use concise and surface
security decisions when trust changes or a package requests execution authority.

Registry design must provide immutable releases, publisher authentication,
strong MFA, namespace ownership, typosquatting defenses, yanking/deprecation,
attestable or signed provenance and security advisories. Dependency confusion
requires explicit origin binding, not popularity-based registry precedence.
These are requirements for future infrastructure, not services deployed today.

## Stable requirements and open decisions

Stable requirements are the three principles above, one CLI, version-free
imports, manifest/lock separation, explicit updates, deterministic locked bytes
and graph, content verification, immutable releases, no default package execution,
explicit constrained authority, content-addressed storage, configurable release
age, visible trust downgrades and auditable dependency paths.

Open decisions include:

- Future improvements beyond bounded single-version SemVer backtracking.
- Coexistence and import mapping for incompatible major versions.
- Production default minimum release age (bootstrap uses zero, project/global maximum).
- Signing format and key lifecycle.
- Provenance format and verification policy.
- Registry federation and origin configuration.
- Sandbox implementation and supported enforcement platforms.
- Package features/configuration model and its graph/artifact identity.

Do not implement a resolver, registry, package manager or sandbox merely to fill
these gaps. Future implementation needs its own reviewed design and validation.

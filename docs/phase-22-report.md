# Phase 22: initial verified source package manager

Implemented the integrated CLI, manifest and exact lock, bounded single-version
SemVer backtracking, origin binding, immutable local publication, HTTPS download,
content-addressed source cache, offline verified compilation and package-scoped
module loading. Compiler imports carry an explicit target map into the existing
resolver; AST and ownership phases retain their established model. Root entry
selection honors the manifest, and native entry selection rejects dependency
`main` functions as application entry points.

Security checks cover the full locked graph and source inventory, normalized
origins, cached/downloaded hashes, symlinks, traversal, unexpected files, unknown
manifest permissions, minimum age and unavailable required provenance. No package
installation/build hooks execute. The compiler consumes checked source snapshots;
editor overlays cannot substitute locked dependency bytes. Watch inputs include
manifest, lock, global policy and dependency inventories.

Validation includes the public CLI workflow from independent directories,
transitive backtracking, identical local module names in different dependencies,
explicit updates/removal/fetch, lock preservation during builds, offline use after
registry removal, tampered cache refusal, immutable release evidence changes,
TLS validation and explicit test CA, policy enforcement, unknown/vulnerable audit
results and dependency-main rejection. Focused library tests cover requirement
syntax, unsafe manifests/paths, SHA-256 known answer and policy merging.

The bootstrap registry has no default public service, publisher identity,
signatures, authenticated network publication, yanking protocol, multi-major
coexistence, package features, build sandbox or artifact cache. Required signing
refuses; integrity verification explicitly reports unknown provenance, and audit
returns a distinct non-success status for unknown evidence. Release age uses
registry-declared timestamps. See [the guide](packages.md) for concrete workflows,
protocol, storage, limits and exit statuses.

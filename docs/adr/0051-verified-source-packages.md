# ADR 0051: verified source packages

Status: accepted for the initial Phase 22 subset.

## Decision

Implement one integrated package CLI over a `tarn_packages` host library. Packages
are pure source data. Never execute installation/build hooks, package commands,
plugins or package-declared link grants. Unknown manifest fields (including build
permissions) fail explicitly. The existing compiler/system-C toolchain boundary
remains separate; no runtime application sandbox is claimed.

`tarn.toml` stores a package name, SemVer version, relative entry, optional registry,
direct dependencies and security policy. Editable TOML preserves comments. Exact,
caret, tilde, wildcard and comma-separated comparator ranges use the established
SemVer library, including its prerelease rules. Package names are lowercase Tarn
identifiers and cannot overlap bundled stdlib names.

`tarn.lock` is versioned canonical JSON containing semantic manifest identity,
exact versions, explicit normalized origins, content hashes, declared release times,
file inventories and exact graph edges. One version/origin per package is allowed.
Add/remove prefer existing compatible locks; updates resolve highest compatible
versions with deterministic bounded backtracking; never
silently substitute an origin or permit incompatible-major duplication. Cycles,
conflicts and exhausted resolution limits are explicit errors. `update <name>`
freezes all other versions; it refuses rather than quietly updating them.

`init`, `add`, `remove`, `update`, `fetch`, `deps`, `verify`, `audit` and `publish`
share `tarn`. Only explicit graph operations write the lock. Normal compilation
is offline and verifies every locked source before loading it; missing/incompatible
locks or caches require explicit update/fetch. Manifestless local programs continue
to work. Import strings remain version-free. Compiler module imports receive an
explicit target map; dependency-local modules have private package namespaces.
Application code can see only direct packages, and a package only its declared
edges. Local/package ambiguity is rejected; packages never acquire stdlib trust.

The bootstrap registry stores immutable `<name>/<version>/release.json` plus
`files/` and a version-list index. Local publication uses no authentication service
and is for explicitly selected, filesystem-controlled registries. HTTPS consumption
uses the same format through system curl, with TLS validation, no redirects,
no curlrc/credential environment and bounded reads/time. There is no default public
registry or network publication API: accounts/MFA/namespaces/signing need separate
registry infrastructure, not fabricated services. `publish` works for local origins
and refuses HTTPS publication explicitly.

Source storage is content-addressed below `TARN_HOME` (default `~/.tarn`), verified
on every use, and contains only the locked inventory. Reject symlinks, traversal,
unsafe path spellings, unexpected files and hash/manifest disagreement. Atomic
same-directory replacement and an exclusive project guard serialize mutations;
interrupted manifest/lock edits fail closed on the next read rather than building
a mixed graph. Normal builds never refresh registry metadata or rewrite locks.

Global/project minimum release age combine by maximum; required provenance combines
by OR. Default age is explicitly zero for this bootstrap registry. Release age
uses the locked registry-declared timestamp, not a signed timestamp guarantee.
Publisher/signature provenance is unknown in this subset; requiring it refuses.
Trust inspection and audit report that uncertainty, never equating a hash match
with trusted code. Advisories are origin-bound registry data; missing evidence is
unknown and produces a distinct non-success audit status. No downgrade from a
previously stronger format is accepted silently.

## Dependencies

Use focused existing standards rather than handwritten TOML/SemVer/cryptography:
`toml_edit` 0.25.15 (parse/display only, preserving comments), `semver` 1.0.28
(no optional serde), and `sha2` 0.10.9 (no assembly). Their locked transitive weight
is 14 added external crates, reviewed with `cargo tree`; parser features exclude debug/serde extras. Reuse
workspace `serde_json` and `url` for registry/lock JSON and URL origin normalization.
System curl is a documented trusted download tool, never package-supplied code.
No async HTTP client/TLS framework is added to the compiler host graph.

## Deliberate limits

No authenticated public registry deployment, network publish, signing format,
package features, multiple-major coexistence, package build sandbox or artifact
cache optimization. Dependencies do not receive execution authority; this removes
hook execution instead of pretending an unenforced sandbox exists. Limits are
explicitly reported and do not weaken requested policy.

## Compiler boundary

Compiler crates do not depend on `tarn_packages`. The host tools (CLI, LSP)
load and verify the manifest, lock and cache, then pass the driver a plain
`PackageSet`: verified source bytes by path, each package's entry and declared
dependencies, the application's direct dependencies, and extra watch inputs.
The driver maps imports to private `@package/...` module identities from that
data alone. This keeps TOML, SemVer, SHA-256 and URL handling out of the
compiler's dependency graph (AGENTS.md dependency policy) while preserving one
import-resolution implementation. `tarn_packages` depends on the driver for
the `PackageSet` type, never the reverse. Driver entry points without
packages keep compiling manifestless programs unchanged.


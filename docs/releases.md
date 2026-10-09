# Linux toolchain releases

GitHub Actions checks the workspace on main and pull requests. A `vX.Y.Z` tag
runs the same full tests, builds Tarn and Tarn LSP on Ubuntu 22.04, exercises the
installer, packages the binaries and publishes a GitHub prerelease. The version
must match Cargo.toml. Release assets are never replaced by the workflow.
Formatting currently reports existing debt without blocking releases.

The supported binary platform is Linux x86_64 with glibc 2.35 or newer.
Native application compilation additionally requires `cc` and libc development
headers. Particular APIs may require native libraries, e.g. OpenSSL or libdbus.
The standard library and runtime are embedded in the compiler.

Download the installer, inspect it, then run it:

```sh
curl --fail --proto '=https' https://tarn-lng.github.io/website/install.sh -o install-tarn.sh
sh install-tarn.sh --version 0.0.1
```

The default directory is `$HOME/.local/bin`. Use `--prefix /absolute/directory`
to choose another location. No sudo or shell-profile modification is performed.
`--force` explicitly replaces unowned or modified binaries. Repeating the
command updates an unchanged managed installation. `--uninstall` removes only
an unchanged managed installation, preserving modified binaries.

SHA256SUMS detects corruption; it is retrieved from the same GitHub release and
is not an independent publisher signature. The installer validates the archive
inventory and compiler version before installing. Each binary replacement is
atomic; replacing the two binaries together is not a transactional operation.

To publish, update the workspace version and lock, run checks, commit, and push
the matching tag. Manual workflow dispatch can retry an existing tag before a
release exists. Published tags and release assets must remain immutable.

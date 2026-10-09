#!/bin/sh
set -eu
ROOT=$(pwd)
TEMP=$(mktemp -d)
trap 'rm -rf "$TEMP"' EXIT HUP INT TERM
VERSION=$(target/release/tarn version | cut -d ' ' -f 2)
tools/release/package.sh "$VERSION"
mkdir "$TEMP/mock"
cat > "$TEMP/mock/curl" <<'MOCK'
#!/bin/sh
set -eu
for arg in "$@"; do
  case "$arg" in https://github.com/tarn-lng/tarn/releases/download/*) SOURCE=${arg##*/} ;; esac
  if [ "${PREVIOUS:-}" = -o ]; then DEST=$arg; fi
  PREVIOUS=$arg
done
cp "$FIXTURE/$SOURCE" "$DEST"
MOCK
chmod +x "$TEMP/mock/curl"
export FIXTURE="$ROOT/target/dist"
export PATH="$TEMP/mock:$PATH"
tools/release/install.sh --version "$VERSION" --prefix "$TEMP/bin"
"$TEMP/bin/tarn" init "$TEMP/project"
(cd "$TEMP/project"; "$TEMP/bin/tarn" run)
tools/release/install.sh --version "$VERSION" --prefix "$TEMP/bin"
tools/release/install.sh --prefix "$TEMP/bin" --uninstall
test ! -e "$TEMP/bin/tarn"
printf 'unmanaged' > "$TEMP/bin/tarn"
if tools/release/install.sh --version "$VERSION" --prefix "$TEMP/bin"; then echo 'Unmanaged binary overwritten' >&2; exit 1; fi
rm "$TEMP/bin/tarn"
cp target/dist/SHA256SUMS "$TEMP/sums"
printf 'corruption' >> "target/dist/tarn-$VERSION-linux-x86_64.tar.gz"
if tools/release/install.sh --version "$VERSION" --prefix "$TEMP/bin"; then echo 'Corrupt download accepted' >&2; exit 1; fi
test ! -e "$TEMP/bin/tarn"
tools/release/package.sh "$VERSION"
echo 'PASS: installer, update, uninstall, existing-file protection and corruption rejection'

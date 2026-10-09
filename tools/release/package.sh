#!/bin/sh
set -eu
VERSION=${1:?Usage: package.sh VERSION}
ROOT=$(pwd)
[ "$(target/release/tarn version)" = "tarn $VERSION" ]
mkdir -p target/dist/stage
cp target/release/tarn target/release/tarn-lsp README.md target/dist/stage/
printf 'Tarn is distributed under MIT OR Apache-2.0, as declared in Cargo.toml.\nLicense texts: https://opensource.org/license/mit and https://www.apache.org/licenses/LICENSE-2.0\n' > target/dist/stage/COPYING
tar -czf "target/dist/tarn-$VERSION-linux-x86_64.tar.gz" -C target/dist/stage tarn tarn-lsp README.md COPYING
(cd "$ROOT/target/dist"; sha256sum "tarn-$VERSION-linux-x86_64.tar.gz" > SHA256SUMS)

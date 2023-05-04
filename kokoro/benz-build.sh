#!/bin/bash
set -euo pipefail
IFS=$'\n\t'

#cd "${KOKORO_ARTIFACTS_DIR}/git/shpool" # for running the script in presubmit to test
cd git/benz-build-source >& /dev/null || cd git

echo "$(date --rfc-3339=seconds): updating apt"
sudo DEBIAN_FRONTEND=noninteractive apt-get -q update

echo "$(date --rfc-3339=seconds): installing packages"
sudo DEBIAN_FRONTEND=noninteractive apt-get -q -y install cargo rustc

echo "$(date --rfc-3339=seconds): showing cargo version"
cargo --version

echo "$(date --rfc-3339=seconds): building cargo-deb"
(cd kokoro/tools/cargo-deb ; cargo build --offline)

echo "$(date --rfc-3339=seconds): packaging"
./kokoro/tools/cargo-deb/target/debug/cargo-deb

mv target/debian/*.deb $KOKORO_ARTIFACTS_DIR

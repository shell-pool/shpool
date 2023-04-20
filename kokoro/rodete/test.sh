#!/bin/bash
set -euo pipefail
IFS=$'\n\t'

cd "${KOKORO_ARTIFACTS_DIR}/git/shpool"

echo "$(date --rfc-3339=seconds): updating apt"
sudo DEBIAN_FRONTEND=noninteractive apt-get -q update

echo "$(date --rfc-3339=seconds): installing packages"
sudo DEBIAN_FRONTEND=noninteractive apt-get -q -y install cargo rustc

echo "$(date --rfc-3339=seconds): showing cargo version"
cargo --version

echo "$(date --rfc-3339=seconds): testing"

# The test suite expects this to be set up
export XDG_RUNTIME_DIR=/run/user/$(id -u)
sudo mkdir -p $XDG_RUNTIME_DIR
sudo chown $USER $XDG_RUNTIME_DIR
chmod 700 $XDG_RUNTIME_DIR

cargo test --offline

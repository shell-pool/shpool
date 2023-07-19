#!/bin/bash

rm -rf vendor .cargo

# filtering breaks the errno crate for some reason
# cargo vendor-filterer --platform=x86_64-unknown-linux-gnu

# the latest version breaks our MSRV
cargo update -p toml_edit --precise 0.19.0
cargo update -p toml_datetime --precise 0.6.0

cargo vendor

git checkout .cargo

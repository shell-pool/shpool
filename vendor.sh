#!/bin/bash

rm -rf vendor .cargo
cargo vendor-filterer --platform=x86_64-unknown-linux-gnu

if [ "x${SHPOOL_REGEN_CARGO_DEB_VENDOR}" != "x" ] ; then
  cd kokoro/tools/cargo-deb
  rm -rf vendor .cargo
  cargo vendor-filterer --platform=x86_64-unknown-linux-gnu

  git checkout .cargo
  cd ../../..
fi

git checkout .cargo

#!/bin/bash

rm -rf vendor .cargo
cargo vendor-filterer --platform=x86_64-unknown-linux-gnu
git checkout .cargo

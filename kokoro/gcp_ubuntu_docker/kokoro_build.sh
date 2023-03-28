#!/bin/bash

cd "${KOKORO_ARTIFACTS_DIR}/git/kokoro-codelab-pailes"

cargo build

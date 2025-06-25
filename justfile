
default:
  just --list

[group("cargo")]
[doc("Builds the Rust project in release mode.")]
build:
  cargo build --release

[group("personal")]
[doc("Copies the release binary to a shared location.")]
share:
  #!/usr/bin/env bash
  set -euo pipefail
  if ! [ -d "/tmp/shared" ]; then
    sudo mkdir -p /tmp/shared
  fi
  if [ $(stat -c '%a' /tmp/shared) -ne 777 ]; then
    sudo chmod 777 /tmp/shared
  fi
  if [ -f "/tmp/shared/pseudo-term" ]; then
    sudo rm -f /tmp/shared/pseudo-term
  fi
  just build
  cp target/release/pseudo-term /tmp/shared/pseudo-term


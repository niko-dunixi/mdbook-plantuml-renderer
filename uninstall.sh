#!/usr/bin/env bash
set -ex
installed_bin="${HOME}/.cargo/bin/mdbook-plantuml-renderer"
if [ -f "${installed_bin}" ]; then
  echo "Removing installed binary"
  rm "${installed_bin}"
fi

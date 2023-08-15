#!/bin/bash

set -euo pipefail

if [ $# -ne 1 ]; then
  exit 1
fi

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -q -y --profile minimal --default-toolchain "$1"

cat <<'EOF' > ./env
if [[ $CARGO_HOME ]]; then
  export PATH="$PATH:$CARGO_HOME/bin"
else
  export PATH="$PATH:$HOME/.cargo/bin"
fi
EOF

#!/bin/bash

set -euo pipefail

. /etc/os-release

case "$ID" in
  debian | ubuntu)
    apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl gcc gcc-multilib webkit2gtk-4.1-dev
    ;;
  fedora)
    dnf install --setopt=install_weak_deps=False -y \
      curl gcc webkit2gtk4.1-devel
    ;;
  *)
    echo "unknown distro"
    exit 1
    ;;
esac

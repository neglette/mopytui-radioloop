#!/bin/sh
# Local SOCKS5 profile for this Radio Loop installation only.
set -eu
cd -- "$(dirname -- "$0")"
MOPYTUI_RADIO_PROXY="${MOPYTUI_RADIO_PROXY:-socks5h://127.0.0.1:1080}"
export MOPYTUI_RADIO_PROXY
export https_proxy="$MOPYTUI_RADIO_PROXY" HTTPS_PROXY="$MOPYTUI_RADIO_PROXY"
export no_proxy="localhost,127.0.0.1" NO_PROXY="localhost,127.0.0.1"
exec ./mopytui "$@"

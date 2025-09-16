#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 --node <1|2|3|4> [--release]" >&2
  exit 1
}

NODE=""
RELEASE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --node|-n)
      NODE="$2"; shift 2 ;;
    --release)
      RELEASE="--release"; shift ;;
    *)
      echo "Unknown arg: $1" >&2; usage ;;
  esac
done

[[ -z "$NODE" ]] && usage
if ! [[ "$NODE" =~ ^[1-4]$ ]]; then
  echo "--node must be 1..4" >&2
  exit 2
fi

P2P_PORT=$((9000 + NODE))
HTTP_PORT=$((8080 + NODE))
KEY_FILE="node${NODE}.key"

# Build peers list (other nodes)
PEERS=()
for n in 1 2 3 4; do
  if [[ "$n" != "$NODE" ]]; then
    PEERS+=(--peers "127.0.0.1:$((9000 + n))")
  fi
done

# Bootstrap keys: include all four to ensure identical genesis set across nodes
BOOTSTRAP_KEYS=(--bootstrap-keys node1.key --bootstrap-keys node2.key --bootstrap-keys node3.key --bootstrap-keys node4.key)

echo "Running node $NODE (P2P=127.0.0.1:${P2P_PORT}, HTTP=127.0.0.1:${HTTP_PORT})"
RUST_LOG=${RUST_LOG:-info} cargo run -p hs-demo ${RELEASE} -- \
  --key-file "${KEY_FILE}" \
  "${BOOTSTRAP_KEYS[@]}" \
  --p2p-listen "127.0.0.1:${P2P_PORT}" \
  --http-listen "127.0.0.1:${HTTP_PORT}" \
  "${PEERS[@]}"


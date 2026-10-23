#!/usr/bin/env bash
set -euo pipefail

# Consistent local Explore ports:
# API 127.0.0.1:8792; Vite 127.0.0.1:5199; 8787 is reserved for OmO Board.
# Override BIND_ADDRESS (complete host:port) and set EXPLORE_API_TARGET to match.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
command -v cargo >/dev/null || { echo "Cargo is required." >&2; exit 1; }

export RUST_LOG="${RUST_LOG:-info}"
export BIND_ADDRESS="${BIND_ADDRESS:-127.0.0.1:8792}"
if [[ -z "${OSM_DB_PATH:-}" && -f "$ROOT/data/osm-bulgaria.db" ]]; then
  export OSM_DB_PATH="$ROOT/data/osm-bulgaria.db"
fi

BASE="http://$BIND_ADDRESS"
if curl -fsS --max-time 3 "$BASE/api/health" | grep -q '"NightKnight Explore API"'; then
  echo "NightKnight Explore API already running at $BASE"
  exit 0
fi

echo "Starting NightKnight Explore API on $BIND_ADDRESS"
(cd "$ROOT/crates/api" && cargo run --bin nightknight-explore-api) &
API_PID=$!
echo "$API_PID" > "$ROOT/.api-pid"

for _ in $(seq 1 120); do
  if curl -fsS --max-time 2 "$BASE/api/health" | grep -q '"NightKnight Explore API"'; then
    echo "Explore API ready at $BASE"
    exit 0
  fi
  if ! kill -0 "$API_PID" 2>/dev/null; then
    echo "Explore API exited before becoming ready." >&2
    exit 1
  fi
  sleep 1
done
echo "Explore API did not become ready; check its log." >&2
exit 1

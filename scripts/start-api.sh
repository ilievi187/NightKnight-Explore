#!/usr/bin/env bash
set -e

# Start the NightKnight Explore Rust API server
# Binds to 127.0.0.1:8787 by default; override BIND_ADDRESS and PORT as needed

cd "$(dirname "$0")/.."

if ! command -v cargo &> /dev/null; then
  echo "Error: Cargo is not installed. See https://rustup.rs"
  exit 1
fi

export RUST_LOG=${RUST_LOG:-info}
export OVERPASS_ENDPOINT=${OVERPASS_ENDPOINT:-}
export GEOCODER_ENDPOINT=${GEOCODER_ENDPOINT:-}

BIND=${BIND_ADDRESS:-127.0.0.1}
PORT=${PORT_NUMBER:-8787}

echo "Starting NightKnight Explore API on $BIND:$PORT"
cargo run --bin nightknight-explore-api -- --bind-address "${BIND}:${PORT}" ${OVERPASS_ENDPOINT:+--overpass-endpoint "$OVERPASS_ENDPOINT"} ${GEOCODER_ENDPOINT:+--geocoder-endpoint "$GEOCODER_ENDPOINT"} &
API_PID=$!
echo "API started (PID $API_PID)"
echo "Save PID to file: .api-pid"
echo $API_PID > .api-pid

# Wait for server to be ready
for i in $(seq 1 30); do
  if curl -s "http://${BIND}:${PORT}/api/health" > /dev/null 2>&1; then
    echo "API is ready."
    exit 0
  fi
  sleep 1
done

echo "Warning: API did not become ready within 30 seconds"
exit 0

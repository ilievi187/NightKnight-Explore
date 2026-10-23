#!/usr/bin/env bash
set -e

# Start the NightKnight Explore Web frontend
# Requires the API server to be running first

cd "$(dirname "$0")/.."

if ! command -v npm &> /dev/null; then
  echo "Error: npm is not installed. See https://nodejs.org"
  exit 1
fi

# Install dependencies if needed
if [ ! -d "apps/web/node_modules" ]; then
  echo "Installing web dependencies..."
  npm install --no-audit --no-fund
fi

echo "Starting NightKnight Explore Web interface"
cd apps/web && npm run dev -- --port 5199 &
WEB_PID=$!
echo "Web started (PID $WEB_PID)"
echo "Save PID to file: .web-pid"
echo $WEB_PID > .web-pid

# Wait for dev server to be ready
for i in $(seq 1 30); do
  if curl -s "http://127.0.0.1:5199/" > /dev/null 2>&1; then
    echo "Web interface is ready."
    echo "Open http://127.0.0.1:5199/ in your browser"
    exit 0
  fi
  sleep 1
done

echo "Warning: Web interface did not become ready within 30 seconds"
exit 0

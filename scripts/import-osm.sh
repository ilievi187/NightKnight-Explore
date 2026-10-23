#!/usr/bin/env bash
set -e

# Download and import OpenStreetMap data for local discovery
# Usage: ./scripts/import-osm.sh [output_db_path]

cd "$(dirname "$0")/.."

OUTPUT_DB="${1:-data/osm-bulgaria.db}"
PBF_URL="https://download.geofabrik.de/europe/bulgaria-latest.osm.pbf"
PBF_FILE="/tmp/bulgaria-latest.osm.pbf"

echo "=== NightKnight Explore: OSM Import ==="
echo "Output DB: $OUTPUT_DB"
echo ""

# Create data directory
mkdir -p "$(dirname "$OUTPUT_DB")"

# Download PBF if not already present
if [ ! -f "$PBF_FILE" ]; then
    echo "Downloading Bulgaria PBF from Geofabrik..."
    curl -L -o "$PBF_FILE" "$PBF_URL"
    echo "Download complete."
else
    echo "PBF file already present at $PBF_FILE"
fi

# Build import tool
echo "Building import tool..."
cargo build --bin import-osm --release 2>&1 | tail -3

# Run import
echo "Importing OSM data..."
./target/release/import-osm "$PBF_FILE" "$OUTPUT_DB"

echo ""
echo "=== Import complete ==="
echo "Set OSM_DB_PATH=$OUTPUT_DB when starting the API to use local data."
#!/bin/bash
# Refresh Materialized Views - Polymarket P&L Substreams
#
# This script refreshes all materialized views concurrently (non-blocking)
# Concurrent refresh allows queries to continue using the views during refresh
#
# Usage:
#   DATABASE_URL="postgres://..." ./scripts/refresh-mat-views.sh
#
# Recommended: Run via cron every 5-15 minutes
#   */10 * * * * cd /path/to/project && ./scripts/refresh-mat-views.sh >> logs/refresh.log 2>&1

set -e  # Exit on error

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL environment variable not set"
    echo "Usage: DATABASE_URL=\"postgres://...\" $0"
    exit 1
fi

# Start time for logging
START_TIME=$(date '+%Y-%m-%d %H:%M:%S')
echo "[$START_TIME] Starting materialized view refresh..."

# Refresh leaderboard_pnl
echo "Refreshing leaderboard_pnl..."
psql "$DATABASE_URL" -c "REFRESH MATERIALIZED VIEW CONCURRENTLY leaderboard_pnl;" || {
    echo "Error: Failed to refresh leaderboard_pnl"
    echo "Note: First refresh must use REFRESH MATERIALIZED VIEW without CONCURRENTLY"
    exit 1
}
echo "✓ leaderboard_pnl refreshed"

# Refresh leaderboard_volume
echo "Refreshing leaderboard_volume..."
psql "$DATABASE_URL" -c "REFRESH MATERIALIZED VIEW CONCURRENTLY leaderboard_volume;" || {
    echo "Error: Failed to refresh leaderboard_volume"
    exit 1
}
echo "✓ leaderboard_volume refreshed"

# Refresh whale_trades
echo "Refreshing whale_trades..."
psql "$DATABASE_URL" -c "REFRESH MATERIALIZED VIEW CONCURRENTLY whale_trades;" || {
    echo "Error: Failed to refresh whale_trades"
    exit 1
}
echo "✓ whale_trades refreshed"

# End time for logging
END_TIME=$(date '+%Y-%m-%d %H:%M:%S')
echo "[$END_TIME] All materialized views refreshed successfully"

# Optional: Display row counts
echo ""
echo "Current row counts:"
psql "$DATABASE_URL" -c "
SELECT
    'leaderboard_pnl' as view_name, COUNT(*) as row_count FROM leaderboard_pnl
UNION ALL
SELECT 'leaderboard_volume', COUNT(*) FROM leaderboard_volume
UNION ALL
SELECT 'whale_trades', COUNT(*) FROM whale_trades;
"

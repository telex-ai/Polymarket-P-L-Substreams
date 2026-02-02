-- Polymarket P&L Substreams - PostgreSQL Schema
-- Version: 1.0.0
--
-- Run with: substreams-sink-sql setup "postgres://..." polymarket-pnl-v1.0.0.spkg

-------------------------------------------------
-- TRADES TABLE: All order fills
-------------------------------------------------
CREATE TABLE IF NOT EXISTS trades (
    id VARCHAR(128) PRIMARY KEY,              -- tx_hash-log_index
    block_number BIGINT NOT NULL,
    block_timestamp TIMESTAMP NOT NULL,
    tx_hash VARCHAR(66) NOT NULL,
    log_index INTEGER NOT NULL,

    -- Participants
    maker VARCHAR(42) NOT NULL,
    taker VARCHAR(42) NOT NULL,

    -- Trade details
    token_id VARCHAR(78) NOT NULL,            -- Outcome token ID
    side VARCHAR(4) NOT NULL,                 -- 'buy' or 'sell'
    price NUMERIC(20, 18) NOT NULL,           -- Execution price (0-1)
    amount NUMERIC(38, 6) NOT NULL,           -- USDC amount (6 decimals)
    fee NUMERIC(38, 6) NOT NULL,              -- Fee in USDC

    -- Exchange info
    exchange VARCHAR(10) NOT NULL,            -- 'ctf' or 'neg_risk'
    order_hash VARCHAR(66),

    -- Indexes for common queries
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trades_block ON trades(block_number);
CREATE INDEX IF NOT EXISTS idx_trades_maker ON trades(maker);
CREATE INDEX IF NOT EXISTS idx_trades_taker ON trades(taker);
CREATE INDEX IF NOT EXISTS idx_trades_token ON trades(token_id);
CREATE INDEX IF NOT EXISTS idx_trades_timestamp ON trades(block_timestamp);

-------------------------------------------------
-- USER_PNL TABLE: Aggregated P&L per user
-------------------------------------------------
CREATE TABLE IF NOT EXISTS user_pnl (
    user_address VARCHAR(42) PRIMARY KEY,

    -- P&L Metrics
    realized_pnl NUMERIC(38, 6) NOT NULL DEFAULT 0,      -- Total realized P&L
    unrealized_pnl NUMERIC(38, 6) NOT NULL DEFAULT 0,    -- Current unrealized P&L
    total_pnl NUMERIC(38, 6) NOT NULL DEFAULT 0,         -- realized + unrealized

    -- Trading Stats
    total_volume NUMERIC(38, 6) NOT NULL DEFAULT 0,      -- Total traded volume
    total_trades INTEGER NOT NULL DEFAULT 0,              -- Number of trades
    total_fees_paid NUMERIC(38, 6) NOT NULL DEFAULT 0,   -- Total fees

    -- Performance Metrics
    win_count INTEGER NOT NULL DEFAULT 0,                 -- Profitable positions closed
    loss_count INTEGER NOT NULL DEFAULT 0,                -- Losing positions closed
    win_rate NUMERIC(5, 2) DEFAULT 0,                     -- win_count / total closed

    -- Risk Metrics
    max_drawdown NUMERIC(38, 6) DEFAULT 0,               -- Maximum drawdown
    largest_win NUMERIC(38, 6) DEFAULT 0,
    largest_loss NUMERIC(38, 6) DEFAULT 0,

    -- Timestamps
    first_trade_at TIMESTAMP,
    last_trade_at TIMESTAMP,
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_user_pnl_total ON user_pnl(total_pnl DESC);
CREATE INDEX IF NOT EXISTS idx_user_pnl_volume ON user_pnl(total_volume DESC);
CREATE INDEX IF NOT EXISTS idx_user_pnl_trades ON user_pnl(total_trades DESC);

-------------------------------------------------
-- USER_POSITIONS TABLE: Current positions per user/token
-------------------------------------------------
CREATE TABLE IF NOT EXISTS user_positions (
    id VARCHAR(164) PRIMARY KEY,              -- user_address-token_id
    user_address VARCHAR(42) NOT NULL,
    token_id VARCHAR(78) NOT NULL,

    -- Position details
    quantity NUMERIC(38, 18) NOT NULL DEFAULT 0,         -- Current holding
    avg_entry_price NUMERIC(20, 18) NOT NULL DEFAULT 0,  -- Weighted avg price
    total_cost_basis NUMERIC(38, 6) NOT NULL DEFAULT 0,  -- Total spent

    -- P&L for this position
    realized_pnl NUMERIC(38, 6) NOT NULL DEFAULT 0,
    unrealized_pnl NUMERIC(38, 6) NOT NULL DEFAULT 0,

    -- Current value
    current_price NUMERIC(20, 18) DEFAULT 0,
    current_value NUMERIC(38, 6) DEFAULT 0,

    -- Timestamps
    opened_at TIMESTAMP,
    last_updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_positions_user ON user_positions(user_address);
CREATE INDEX IF NOT EXISTS idx_positions_token ON user_positions(token_id);
CREATE INDEX IF NOT EXISTS idx_positions_quantity ON user_positions(quantity) WHERE quantity > 0;

-------------------------------------------------
-- MARKETS TABLE: Market/Token statistics
-------------------------------------------------
CREATE TABLE IF NOT EXISTS markets (
    token_id VARCHAR(78) PRIMARY KEY,

    -- Market info (populated from TokenRegistered events)
    condition_id VARCHAR(66),
    is_neg_risk BOOLEAN DEFAULT FALSE,

    -- Trading stats
    total_volume NUMERIC(38, 6) NOT NULL DEFAULT 0,
    total_trades INTEGER NOT NULL DEFAULT 0,
    unique_traders INTEGER NOT NULL DEFAULT 0,

    -- Price data
    current_price NUMERIC(20, 18) DEFAULT 0,
    price_24h_ago NUMERIC(20, 18) DEFAULT 0,
    price_change_24h NUMERIC(10, 4) DEFAULT 0,
    high_24h NUMERIC(20, 18) DEFAULT 0,
    low_24h NUMERIC(20, 18) DEFAULT 0,

    -- Timestamps
    first_trade_at TIMESTAMP,
    last_trade_at TIMESTAMP,
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_markets_volume ON markets(total_volume DESC);
CREATE INDEX IF NOT EXISTS idx_markets_trades ON markets(total_trades DESC);

-------------------------------------------------
-- DAILY_STATS TABLE: Daily aggregated statistics
-------------------------------------------------
CREATE TABLE IF NOT EXISTS daily_stats (
    date DATE PRIMARY KEY,

    -- Volume
    total_volume NUMERIC(38, 6) NOT NULL DEFAULT 0,
    total_fees NUMERIC(38, 6) NOT NULL DEFAULT 0,

    -- Activity
    total_trades INTEGER NOT NULL DEFAULT 0,
    unique_traders INTEGER NOT NULL DEFAULT 0,
    new_traders INTEGER NOT NULL DEFAULT 0,

    -- P&L
    total_realized_pnl NUMERIC(38, 6) NOT NULL DEFAULT 0,
    profitable_traders INTEGER NOT NULL DEFAULT 0,
    losing_traders INTEGER NOT NULL DEFAULT 0,

    updated_at TIMESTAMP DEFAULT NOW()
);

-------------------------------------------------
-- LEADERBOARD VIEW: Top traders by P&L
-------------------------------------------------
CREATE OR REPLACE VIEW leaderboard_pnl AS
SELECT
    user_address,
    total_pnl,
    realized_pnl,
    unrealized_pnl,
    total_volume,
    total_trades,
    win_rate,
    RANK() OVER (ORDER BY total_pnl DESC) as rank
FROM user_pnl
WHERE total_trades >= 5  -- Minimum trades to qualify
ORDER BY total_pnl DESC
LIMIT 1000;

-------------------------------------------------
-- LEADERBOARD VIEW: Top traders by volume
-------------------------------------------------
CREATE OR REPLACE VIEW leaderboard_volume AS
SELECT
    user_address,
    total_volume,
    total_trades,
    total_pnl,
    win_rate,
    RANK() OVER (ORDER BY total_volume DESC) as rank
FROM user_pnl
WHERE total_trades >= 1
ORDER BY total_volume DESC
LIMIT 1000;

-------------------------------------------------
-- WHALE TRADES VIEW: Large trades (>$10K)
-------------------------------------------------
CREATE OR REPLACE VIEW whale_trades AS
SELECT
    t.*,
    u.total_pnl as trader_pnl,
    u.win_rate as trader_win_rate
FROM trades t
LEFT JOIN user_pnl u ON t.taker = u.user_address
WHERE t.amount >= 10000
ORDER BY t.block_timestamp DESC
LIMIT 1000;

-------------------------------------------------
-- FUNCTIONS
-------------------------------------------------

-- Function to calculate win rate
CREATE OR REPLACE FUNCTION calculate_win_rate(wins INTEGER, losses INTEGER)
RETURNS NUMERIC AS $$
BEGIN
    IF (wins + losses) = 0 THEN
        RETURN 0;
    END IF;
    RETURN ROUND((wins::NUMERIC / (wins + losses)::NUMERIC) * 100, 2);
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-update win_rate
CREATE OR REPLACE FUNCTION update_win_rate()
RETURNS TRIGGER AS $$
BEGIN
    NEW.win_rate := calculate_win_rate(NEW.win_count, NEW.loss_count);
    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_update_win_rate ON user_pnl;
CREATE TRIGGER trigger_update_win_rate
    BEFORE INSERT OR UPDATE ON user_pnl
    FOR EACH ROW
    EXECUTE FUNCTION update_win_rate();

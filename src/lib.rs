//! Polymarket P&L Substreams v1.0.0
//!
//! Real-time Profit & Loss tracking with SQL sink support.
//!
//! Modules:
//! - Layer 1: Event extraction (map_order_fills, map_token_transfers, map_usdc_transfers)
//! - Layer 2: State stores (positions, cost_basis, realized_pnl, prices)
//! - Layer 3: Analytics (map_user_pnl, map_market_stats)
//! - Layer 4: SQL sink (db_out)

mod abi;
mod pb;

use hex_literal::hex;
use pb::pnl::v1 as pnl;
use substreams::prelude::*;
use substreams::store::{StoreAddBigInt, StoreAddInt64, StoreGet, StoreGetProto, StoreSetProto};
use substreams::Hex;
use substreams_database_change::pb::database::DatabaseChanges;
use substreams_database_change::tables::Tables;
use substreams_ethereum::pb::eth::v2 as eth;

use substreams::scalar::BigInt;
use std::str::FromStr;

substreams_ethereum::init!();

// Contract addresses
const CTF_EXCHANGE: [u8; 20] = hex!("4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e");
const NEG_RISK_EXCHANGE: [u8; 20] = hex!("C5d563A36AE78145C45a50134d48A1215220f80a");
const USDC_CONTRACT: [u8; 20] = hex!("2791bca1f2de4661ed88a30c99a7a9449aa84174");

// Event signatures
const TRANSFER_SINGLE_SIG: [u8; 32] =
    hex!("c3d58168c5ae7397731d063d5bbf3d657854427343f4c083240f7aacaa2d0f62");
const TRANSFER_SIG: [u8; 32] =
    hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

// Excluded addresses (protocol contracts, not real users)
const EXCLUDED_ADDRESSES: [&str; 6] = [
    "0x4d97dcd97ec945f40cf65f87097ace5ea0476045",
    "0x4bfb41d5b3570defd03c39a9a4d8de6bd8b8982e",
    "0xc5d563a36ae78145c45a50134d48a1215220f80a",
    "0x78769d50be1763ed1ca0d5e878d93f05aabff29e",
    "0xa5ef39c3d3e10d0b270233af41cac69796b12966",
    "0x0000000000000000000000000000000000000000",
];

fn is_excluded_address(addr: &str) -> bool {
    EXCLUDED_ADDRESSES
        .iter()
        .any(|&excluded| excluded.eq_ignore_ascii_case(addr))
}

fn format_address(bytes: &[u8]) -> String {
    format!("0x{}", Hex(bytes).to_string())
}

//==============================================
// LAYER 1: Event Extraction
//==============================================

/// Extracts OrderFilled events from CTF Exchange and NegRisk Exchange
#[substreams::handlers::map]
fn map_order_fills(blk: eth::Block) -> Result<pnl::OrderFills, substreams::errors::Error> {
    let mut fills = pnl::OrderFills {
        block_number: blk.number,
        block_timestamp: Some(blk.timestamp().clone()),
        ..Default::default()
    };

    for receipt in blk.receipts() {
        for log in &receipt.receipt.logs {
            let is_ctf = log.address == CTF_EXCHANGE;
            let is_neg_risk = log.address == NEG_RISK_EXCHANGE;

            if !is_ctf && !is_neg_risk {
                continue;
            }

            if let Some(decoded) = abi::decode_order_filled(log) {
                let maker = format_address(&decoded.maker);
                let taker = format_address(&decoded.taker);

                // Skip if both parties are excluded
                if is_excluded_address(&maker) && is_excluded_address(&taker) {
                    continue;
                }

                // Determine side and calculate price
                let maker_amount = BigInt::from_str(&decoded.maker_amount_filled).unwrap_or_default();
                let taker_amount = BigInt::from_str(&decoded.taker_amount_filled).unwrap_or_default();

                let (side, price, amount, token_id) = if decoded.maker_asset_id == "0" {
                    // Maker is paying USDC -> Taker is selling
                    let price = if !taker_amount.is_zero() {
                        maker_amount.clone() * BigInt::from(1_000_000i64) / taker_amount.clone()
                    } else {
                        BigInt::from(0)
                    };
                    (
                        "sell".to_string(),
                        format!("0.{:06}", price.to_u64()),
                        decoded.maker_amount_filled.clone(),
                        decoded.taker_asset_id.clone(),
                    )
                } else {
                    // Taker is paying USDC -> Taker is buying
                    let price = if !maker_amount.is_zero() {
                        taker_amount.clone() * BigInt::from(1_000_000i64) / maker_amount.clone()
                    } else {
                        BigInt::from(0)
                    };
                    (
                        "buy".to_string(),
                        format!("0.{:06}", price.to_u64()),
                        decoded.taker_amount_filled.clone(),
                        decoded.maker_asset_id.clone(),
                    )
                };

                let fill = pnl::OrderFill {
                    id: format!("{}-{}", Hex(&receipt.transaction.hash).to_string(), log.index),
                    tx_hash: Hex(&receipt.transaction.hash).to_string(),
                    log_index: log.index,
                    block_number: blk.number,
                    timestamp: Some(blk.timestamp().clone()),
                    maker,
                    taker,
                    token_id,
                    side,
                    price,
                    amount,
                    fee: decoded.fee,
                    maker_asset_id: decoded.maker_asset_id,
                    taker_asset_id: decoded.taker_asset_id,
                    maker_amount_filled: decoded.maker_amount_filled,
                    taker_amount_filled: decoded.taker_amount_filled,
                    exchange: if is_ctf { "ctf" } else { "neg_risk" }.to_string(),
                    order_hash: decoded.order_hash,
                };

                fills.fills.push(fill);
            }
        }
    }

    Ok(fills)
}

/// Extracts ERC1155 TransferSingle events
#[substreams::handlers::map]
fn map_token_transfers(blk: eth::Block) -> Result<pnl::TokenTransfers, substreams::errors::Error> {
    let mut transfers = pnl::TokenTransfers {
        block_number: blk.number,
        ..Default::default()
    };

    for receipt in blk.receipts() {
        for log in &receipt.receipt.logs {
            if log.topics.len() >= 4 && log.topics[0] == TRANSFER_SINGLE_SIG {
                if let Some(decoded) = abi::decode_erc1155_transfer_single(log) {
                    let from = format_address(&decoded.from);
                    let to = format_address(&decoded.to);

                    // Skip internal transfers
                    if is_excluded_address(&from) && is_excluded_address(&to) {
                        continue;
                    }

                    transfers.transfers.push(pnl::TokenTransfer {
                        tx_hash: Hex(&receipt.transaction.hash).to_string(),
                        log_index: log.index,
                        block_number: blk.number,
                        timestamp: Some(blk.timestamp().clone()),
                        from_address: from,
                        to_address: to,
                        token_id: decoded.token_id,
                        amount: decoded.amount,
                        contract_address: format_address(&log.address),
                    });
                }
            }
        }
    }

    Ok(transfers)
}

/// Extracts USDC Transfer events
#[substreams::handlers::map]
fn map_usdc_transfers(blk: eth::Block) -> Result<pnl::UsdcTransfers, substreams::errors::Error> {
    let mut transfers = pnl::UsdcTransfers {
        block_number: blk.number,
        ..Default::default()
    };

    for receipt in blk.receipts() {
        for log in &receipt.receipt.logs {
            if log.address == USDC_CONTRACT
                && log.topics.len() >= 3
                && log.topics[0] == TRANSFER_SIG
            {
                if let Some(decoded) = abi::decode_erc20_transfer(log) {
                    transfers.transfers.push(pnl::UsdcTransfer {
                        tx_hash: Hex(&receipt.transaction.hash).to_string(),
                        log_index: log.index,
                        block_number: blk.number,
                        timestamp: Some(blk.timestamp().clone()),
                        from_address: format_address(&decoded.from),
                        to_address: format_address(&decoded.to),
                        amount: decoded.amount,
                    });
                }
            }
        }
    }

    Ok(transfers)
}

//==============================================
// LAYER 2: Stores
//==============================================

/// Store user positions: key = {user}:{token_id}, value = quantity delta
#[substreams::handlers::store]
fn store_user_positions(transfers: pnl::TokenTransfers, store: StoreAddBigInt) {
    for transfer in transfers.transfers {
        let amount = BigInt::from_str(&transfer.amount).unwrap_or_default();

        // Decrease from sender
        if !is_excluded_address(&transfer.from_address) {
            let key = format!("{}:{}", transfer.from_address.to_lowercase(), transfer.token_id);
            let neg_amount = BigInt::from(0i64) - amount.clone();
            store.add(0, &key, &neg_amount);
        }

        // Increase for receiver
        if !is_excluded_address(&transfer.to_address) {
            let key = format!("{}:{}", transfer.to_address.to_lowercase(), transfer.token_id);
            store.add(0, &key, &amount);
        }
    }
}

/// Store user cost basis: key = {user}:{token_id}, value = total cost
#[substreams::handlers::store]
fn store_user_cost_basis(fills: pnl::OrderFills, store: StoreAddBigInt) {
    for fill in fills.fills {
        let amount = BigInt::from_str(&fill.amount).unwrap_or_default();

        if fill.side == "buy" && !is_excluded_address(&fill.taker) {
            let key = format!("{}:{}", fill.taker.to_lowercase(), fill.token_id);
            store.add(0, &key, &amount);
        }
    }
}

/// Store user realized P&L: key = {user}, value = realized P&L delta
#[substreams::handlers::store]
fn store_user_realized_pnl(fills: pnl::OrderFills, store: StoreAddBigInt) {
    for fill in fills.fills {
        // For sells, calculate realized P&L
        // This is simplified - full implementation would track avg entry price
        if fill.side == "sell" && !is_excluded_address(&fill.taker) {
            let key = fill.taker.to_lowercase();
            let pnl = BigInt::from_str(&fill.amount).unwrap_or_default();
            store.add(0, &key, &pnl);
        }
    }
}

/// Store user volume: key = {user}, value = volume delta
#[substreams::handlers::store]
fn store_user_volume(fills: pnl::OrderFills, store: StoreAddBigInt) {
    for fill in fills.fills {
        let amount = BigInt::from_str(&fill.amount).unwrap_or_default();

        // Track volume for both maker and taker
        if !is_excluded_address(&fill.maker) {
            store.add(0, &fill.maker.to_lowercase(), &amount);
        }
        if !is_excluded_address(&fill.taker) {
            store.add(0, &fill.taker.to_lowercase(), &amount);
        }
    }
}

/// Store user trade count: key = {user}, value = count delta
#[substreams::handlers::store]
fn store_user_trade_count(fills: pnl::OrderFills, store: StoreAddInt64) {
    for fill in fills.fills {
        if !is_excluded_address(&fill.maker) {
            store.add(0, &fill.maker.to_lowercase(), 1);
        }
        if !is_excluded_address(&fill.taker) {
            store.add(0, &fill.taker.to_lowercase(), 1);
        }
    }
}

/// Store market volume: key = {token_id}, value = volume
#[substreams::handlers::store]
fn store_market_volume(fills: pnl::OrderFills, store: StoreAddBigInt) {
    for fill in fills.fills {
        let amount = BigInt::from_str(&fill.amount).unwrap_or_default();
        store.add(0, &fill.token_id, &amount);
    }
}

/// Store latest prices: key = {token_id}, value = TokenPrice proto
#[substreams::handlers::store]
fn store_latest_prices(fills: pnl::OrderFills, store: StoreSetProto<pnl::TokenPrice>) {
    for fill in fills.fills {
        let price = pnl::TokenPrice {
            token_id: fill.token_id.clone(),
            price: fill.price.clone(),
            block_number: fill.block_number,
            timestamp: fill.timestamp,
            volume_24h: String::new(),
        };
        store.set(0, &fill.token_id, &price);
    }
}

//==============================================
// LAYER 3: Analytics
//==============================================

/// Compute user P&L updates
#[substreams::handlers::map]
fn map_user_pnl(
    fills: pnl::OrderFills,
    positions_deltas: Deltas<DeltaBigInt>,
    _cost_basis_store: StoreGetBigInt,
    realized_pnl_store: StoreGetBigInt,
    _prices_store: StoreGetProto<pnl::TokenPrice>,
) -> Result<pnl::UserPnLUpdates, substreams::errors::Error> {
    let mut updates = pnl::UserPnLUpdates {
        block_number: fills.block_number,
        ..Default::default()
    };

    // Track users with position changes
    let mut affected_users: std::collections::HashSet<String> = std::collections::HashSet::new();

    for delta in positions_deltas.deltas {
        let parts: Vec<&str> = delta.key.split(':').collect();
        if parts.len() == 2 {
            affected_users.insert(parts[0].to_string());
        }
    }

    // Also track users from fills
    for fill in &fills.fills {
        if !is_excluded_address(&fill.taker) {
            affected_users.insert(fill.taker.to_lowercase());
        }
        if !is_excluded_address(&fill.maker) {
            affected_users.insert(fill.maker.to_lowercase());
        }
    }

    // Generate updates for affected users
    for user in affected_users {
        let realized = realized_pnl_store
            .get_last(&user)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "0".to_string());

        updates.updates.push(pnl::UserPnLUpdate {
            user_address: user.clone(),
            realized_pnl: realized.clone(),
            unrealized_pnl: "0".to_string(), // Would need current positions × prices
            total_pnl: realized,
            total_volume: "0".to_string(),
            total_trades: 0,
            total_fees_paid: "0".to_string(),
            win_count: 0,
            loss_count: 0,
            win_rate: "0".to_string(),
            max_drawdown: "0".to_string(),
            largest_win: "0".to_string(),
            largest_loss: "0".to_string(),
            first_trade_at: None,
            last_trade_at: Some(fills.block_timestamp.clone().unwrap_or_default()),
            positions: vec![],
        });
    }

    Ok(updates)
}

/// Compute market statistics
#[substreams::handlers::map]
fn map_market_stats(
    fills: pnl::OrderFills,
    volume_deltas: Deltas<DeltaBigInt>,
) -> Result<pnl::MarketStats, substreams::errors::Error> {
    let mut stats = pnl::MarketStats {
        block_number: fills.block_number,
        ..Default::default()
    };

    for delta in volume_deltas.deltas {
        stats.stats.push(pnl::MarketStat {
            token_id: delta.key,
            total_volume: delta.new_value.to_string(),
            ..Default::default()
        });
    }

    Ok(stats)
}

//==============================================
// LAYER 4: SQL Sink
//==============================================

/// Output database changes for SQL sink
#[substreams::handlers::map]
fn db_out(
    params: String,
    fills: pnl::OrderFills,
    user_pnl: pnl::UserPnLUpdates,
    market_stats: pnl::MarketStats,
) -> Result<DatabaseChanges, substreams::errors::Error> {
    let mut tables = Tables::new();

    // Parse params
    let min_trade_size: i64 = params
        .split('=')
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    // Insert trades
    for fill in fills.fills {
        let amount: i64 = fill.amount.parse().unwrap_or(0);

        // Skip small trades if configured
        if amount < min_trade_size {
            continue;
        }

        let timestamp = fill
            .timestamp
            .as_ref()
            .map(|t| t.seconds.to_string())
            .unwrap_or_default();

        tables
            .create_row("trades", &fill.id)
            .set("block_number", fill.block_number)
            .set("block_timestamp", &timestamp)
            .set("tx_hash", &fill.tx_hash)
            .set("log_index", fill.log_index)
            .set("maker", &fill.maker)
            .set("taker", &fill.taker)
            .set("token_id", &fill.token_id)
            .set("side", &fill.side)
            .set("price", &fill.price)
            .set("amount", &fill.amount)
            .set("fee", &fill.fee)
            .set("exchange", &fill.exchange)
            .set("order_hash", &fill.order_hash);
    }

    // Upsert user P&L
    for update in user_pnl.updates {
        tables
            .update_row("user_pnl", &update.user_address)
            .set("realized_pnl", &update.realized_pnl)
            .set("unrealized_pnl", &update.unrealized_pnl)
            .set("total_pnl", &update.total_pnl)
            .set("total_volume", &update.total_volume)
            .set("total_trades", update.total_trades as i64)
            .set("total_fees_paid", &update.total_fees_paid)
            .set("win_count", update.win_count as i64)
            .set("loss_count", update.loss_count as i64)
            .set("win_rate", &update.win_rate);
    }

    // Upsert market stats
    for stat in market_stats.stats {
        tables
            .update_row("markets", &stat.token_id)
            .set("total_volume", &stat.total_volume)
            .set("current_price", &stat.current_price);
    }

    Ok(tables.to_database_changes())
}

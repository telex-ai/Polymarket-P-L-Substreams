//! ABI decoders for Polymarket contract events

use substreams::Hex;
use substreams_ethereum::pb::eth::v2::Log;

/// Decoded OrderFilled event
pub struct OrderFilledEvent {
    pub order_hash: String,
    pub maker: Vec<u8>,
    pub taker: Vec<u8>,
    pub maker_asset_id: String,
    pub taker_asset_id: String,
    pub maker_amount_filled: String,
    pub taker_amount_filled: String,
    pub fee: String,
}

/// Decoded ERC1155 TransferSingle event
pub struct TransferSingleEvent {
    pub operator: Vec<u8>,
    pub from: Vec<u8>,
    pub to: Vec<u8>,
    pub token_id: String,
    pub amount: String,
}

/// Decoded ERC20 Transfer event
pub struct TransferEvent {
    pub from: Vec<u8>,
    pub to: Vec<u8>,
    pub amount: String,
}

/// OrderFilled event signature: OrderFilled(bytes32,address,address,uint256,uint256,uint256,uint256,uint256)
const ORDER_FILLED_SIG: [u8; 32] = [
    0xd0, 0xa0, 0x8e, 0x8c, 0x49, 0x3f, 0x9c, 0x94, 0xf2, 0x9c, 0xd8, 0x23,
    0xd8, 0x49, 0x1c, 0x59, 0x5b, 0xa2, 0x16, 0x41, 0x3f, 0x5c, 0x5a, 0xf0,
    0xab, 0x29, 0x66, 0x2a, 0x79, 0x5b, 0x4b, 0xa4,
];

/// Decode OrderFilled event from log
pub fn decode_order_filled(log: &Log) -> Option<OrderFilledEvent> {
    // Check event signature
    if log.topics.is_empty() {
        return None;
    }

    // OrderFilled events have specific structure
    // We need at least enough data for the event parameters
    if log.data.len() < 224 {
        // 7 * 32 bytes
        return None;
    }

    // Parse data fields
    let order_hash = Hex(&log.data[0..32]).to_string();
    let maker = log.data[44..64].to_vec(); // Skip 12 bytes padding for address
    let taker = log.data[76..96].to_vec();

    // Parse uint256 values
    let maker_asset_id = parse_uint256(&log.data[96..128]);
    let taker_asset_id = parse_uint256(&log.data[128..160]);
    let maker_amount_filled = parse_uint256(&log.data[160..192]);
    let taker_amount_filled = parse_uint256(&log.data[192..224]);

    let fee = if log.data.len() >= 256 {
        parse_uint256(&log.data[224..256])
    } else {
        "0".to_string()
    };

    Some(OrderFilledEvent {
        order_hash,
        maker,
        taker,
        maker_asset_id,
        taker_asset_id,
        maker_amount_filled,
        taker_amount_filled,
        fee,
    })
}

/// Decode ERC1155 TransferSingle event
/// Event: TransferSingle(address indexed operator, address indexed from, address indexed to, uint256 id, uint256 value)
pub fn decode_erc1155_transfer_single(log: &Log) -> Option<TransferSingleEvent> {
    if log.topics.len() < 4 || log.data.len() < 64 {
        return None;
    }

    let operator = log.topics[1][12..32].to_vec();
    let from = log.topics[2][12..32].to_vec();
    let to = log.topics[3][12..32].to_vec();

    let token_id = parse_uint256(&log.data[0..32]);
    let amount = parse_uint256(&log.data[32..64]);

    Some(TransferSingleEvent {
        operator,
        from,
        to,
        token_id,
        amount,
    })
}

/// Decode ERC20 Transfer event
/// Event: Transfer(address indexed from, address indexed to, uint256 value)
pub fn decode_erc20_transfer(log: &Log) -> Option<TransferEvent> {
    if log.topics.len() < 3 || log.data.len() < 32 {
        return None;
    }

    let from = log.topics[1][12..32].to_vec();
    let to = log.topics[2][12..32].to_vec();
    let amount = parse_uint256(&log.data[0..32]);

    Some(TransferEvent { from, to, amount })
}

/// Parse uint256 from bytes (big-endian)
fn parse_uint256(data: &[u8]) -> String {
    if data.len() != 32 {
        return "0".to_string();
    }

    // Skip leading zeros and convert to decimal string
    let result = num_bigint::BigUint::from_bytes_be(data);
    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uint256() {
        let data = [0u8; 32];
        assert_eq!(parse_uint256(&data), "0");

        let mut data = [0u8; 32];
        data[31] = 1;
        assert_eq!(parse_uint256(&data), "1");

        let mut data = [0u8; 32];
        data[31] = 100;
        assert_eq!(parse_uint256(&data), "100");
    }
}

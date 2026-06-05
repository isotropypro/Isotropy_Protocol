use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

pub const TOKEN_SYMBOL: &str = "isLUNC";
pub const TOKEN_DECIMALS: u8 = 6;
pub const DEFAULT_BURN_DENOM: &str = "uluna";
pub const DEFAULT_CYCLE_DURATION_SECONDS: u64 = 600;
pub const EMISSION_DAYS: u64 = 105_120;
pub const BATCH_SIZE_LUNC: u128 = 10_000_000_000;
pub const MIN_BATCHES: u32 = 1;
pub const MAX_BATCHES: u32 = 10_000;
pub const DEFAULT_PROTOCOL_FEE_RATE_STR: &str = "0.10";
pub const MIN_PROTOCOL_FEE_RATE_STR: &str = "0.0001";
pub const ALLOCATION_FEE_MAX_STR: &str = "1.50";
pub const ALLOCATION_FEE_MIN_STR: &str = "0.20";
pub const DAILY_DECAY_RATE_STR: &str = "0.000006804672818852";
pub const INITIAL_DAILY_EMISSION: u128 = 1_332_820_936;

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub burn_address: String,
    pub protocol_fee_address: String,
    pub protocol_fee_rate: Decimal,
    pub token_code_id: u64,
    pub token_address: Option<Addr>,
    pub burn_denom: String,
    pub cycle_duration_seconds: u64,
}

#[cw_serde]
pub struct GlobalState {
    pub current_cycle_id: u64,
    pub current_cycle_start: u64,
    pub active_cycle_burned: Uint128,
    #[serde(default)]
    pub emitted_cycle_count: u64,
    pub total_supply: Uint128,
    pub total_staked: Uint128,
    pub pending_reward_pool: Uint128,
    pub reward_index: Decimal,
}

#[cw_serde]
pub struct UserPosition {
    pub staked_balance: Uint128,
    pub pending_lunc_rewards: Uint128,
    pub reward_index: Decimal,
    pub next_cycle_to_process: u64,
}

impl Default for UserPosition {
    fn default() -> Self {
        Self {
            staked_balance: Uint128::zero(),
            pending_lunc_rewards: Uint128::zero(),
            reward_index: Decimal::zero(),
            next_cycle_to_process: 1,
        }
    }
}

#[cw_serde]
pub struct CycleSummary {
    pub cycle_id: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub total_burned: Uint128,
    pub emission_amount: Uint128,
    pub reward_pool: Uint128,
    pub reward_index_before: Decimal,
    pub reward_index_after: Decimal,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const GLOBAL_STATE: Item<GlobalState> = Item::new("global_state");
pub const POSITIONS: Map<&Addr, UserPosition> = Map::new("positions");
pub const CYCLE_BURNS: Map<(u64, &Addr), Uint128> = Map::new("cycle_burns");
pub const CYCLE_SUMMARIES: Map<u64, CycleSummary> = Map::new("cycle_summaries");

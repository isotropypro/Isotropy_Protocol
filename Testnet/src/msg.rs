use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Option<String>,
    pub burn_address: String,
    pub protocol_fee_address: String,
    pub protocol_fee_rate: Option<Decimal>,
    pub token_code_id: u64,
    pub token_label: Option<String>,
    pub burn_denom: Option<String>,
    pub cycle_duration_seconds: Option<u64>,
    pub initial_cycle_start_timestamp: Option<u64>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Burn { batches: u32 },
    AdvanceCycle {},
    Receive(Cw20ReceiveMsg),
    Stake { amount: Uint128 },
    Unstake { amount: Uint128 },
    ClaimRewards {},
    UpdateConfig {
        owner: Option<String>,
        burn_address: Option<String>,
        protocol_fee_address: Option<String>,
        protocol_fee_rate: Option<Decimal>,
        cycle_duration_seconds: Option<u64>,
    },
}

#[cw_serde]
pub enum Cw20HookMsg {
    Stake {},
}

#[cw_serde]
pub struct MigrateMsg {}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(GlobalStateResponse)]
    GlobalState {},
    #[returns(BurnQuoteResponse)]
    BurnQuote { batches: u32 },
    #[returns(PositionResponse)]
    Position { address: String },
    #[returns(CycleResponse)]
    Cycle { cycle_id: u64 },
    #[returns(CurrentCycleResponse)]
    CurrentCycle {},
    #[returns(EmissionPointResponse)]
    EmissionPoint { day: u64 },
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,
    pub burn_address: String,
    pub protocol_fee_address: String,
    pub protocol_fee_rate: Decimal,
    pub token_address: Option<String>,
    pub token_code_id: u64,
    pub burn_denom: String,
    pub cycle_duration_seconds: u64,
    pub token_symbol: String,
    pub token_decimals: u8,
    pub daily_decay_rate: Decimal,
}

#[cw_serde]
pub struct GlobalStateResponse {
    pub current_cycle_id: u64,
    pub current_cycle_start: u64,
    pub current_cycle_end: u64,
    pub total_supply: Uint128,
    pub total_staked: Uint128,
    pub active_cycle_burned: Uint128,
    pub pending_reward_pool: Uint128,
    pub reward_index: Decimal,
}

#[cw_serde]
pub struct BurnQuoteResponse {
    pub batches: u32,
    pub burn_amount: Uint128,
    pub protocol_fee_amount: Uint128,
    pub allocation_fee_amount: Uint128,
    pub total_amount: Uint128,
    pub allocation_fee_rate: Decimal,
}

#[cw_serde]
pub struct PositionResponse {
    pub address: String,
    pub liquid_balance: Uint128,
    pub staked_balance: Uint128,
    pub pending_lunc_rewards: Uint128,
    pub current_cycle_burned: Uint128,
    pub next_cycle_to_process: u64,
}

#[cw_serde]
pub struct CycleResponse {
    pub cycle_id: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub total_burned: Uint128,
    pub emission_amount: Uint128,
    pub reward_pool: Uint128,
    pub reward_index_before: Decimal,
    pub reward_index_after: Decimal,
}

#[cw_serde]
pub struct CurrentCycleResponse {
    pub cycle_id: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub total_burned: Uint128,
    pub projected_emission: Uint128,
    pub pending_reward_pool: Uint128,
}

#[cw_serde]
pub struct EmissionPointResponse {
    pub day: u64,
    pub amount: Uint128,
}

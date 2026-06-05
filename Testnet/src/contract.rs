use std::str::FromStr;

use cosmwasm_std::{
    attr, entry_point, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Decimal, Deps,
    DepsMut, Env, MessageInfo, Order, Reply, Response, StdError, StdResult, Uint128, WasmMsg,
};
use cw2::{get_contract_version, set_contract_version};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg};
use cw_utils::parse_reply_instantiate_data;
use isotropy_token::controller_instantiate_msg;

use crate::error::ContractError;
use crate::msg::{
    BurnQuoteResponse, ConfigResponse, CurrentCycleResponse, Cw20HookMsg, CycleResponse,
    EmissionPointResponse, ExecuteMsg, GlobalStateResponse, InstantiateMsg, MigrateMsg,
    PositionResponse, QueryMsg,
};
use crate::state::{
    CycleSummary, Config, GlobalState, UserPosition, ALLOCATION_FEE_MAX_STR, ALLOCATION_FEE_MIN_STR,
    BATCH_SIZE_LUNC, CONFIG, CYCLE_BURNS, CYCLE_SUMMARIES, DAILY_DECAY_RATE_STR,
    DEFAULT_BURN_DENOM, DEFAULT_CYCLE_DURATION_SECONDS, DEFAULT_PROTOCOL_FEE_RATE_STR,
    EMISSION_DAYS, GLOBAL_STATE, INITIAL_DAILY_EMISSION, MAX_BATCHES, MIN_BATCHES,
    MIN_PROTOCOL_FEE_RATE_STR, POSITIONS, TOKEN_DECIMALS, TOKEN_SYMBOL,
};

const DECIMAL_FRACTIONAL: u128 = 1_000_000_000_000_000_000;
const CONTRACT_NAME: &str = "crates.io:isotropy-protocol";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
const TOKEN_INSTANTIATE_REPLY_ID: u64 = 1;

struct AdvanceResult {
    advanced_cycles: u64,
    messages: Vec<CosmosMsg>,
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let owner = match msg.owner {
        Some(owner) => deps.api.addr_validate(&owner)?,
        None => info.sender.clone(),
    };

    if msg.token_code_id == 0 {
        return Err(ContractError::Std(StdError::generic_err("invalid token code id")));
    }

    let cycle_duration_seconds = msg
        .cycle_duration_seconds
        .unwrap_or(DEFAULT_CYCLE_DURATION_SECONDS);
    if cycle_duration_seconds == 0 {
        return Err(ContractError::InvalidCycleDuration);
    }
    let initial_cycle_start_timestamp = msg
        .initial_cycle_start_timestamp
        .unwrap_or_else(|| env.block.time.seconds());
    if initial_cycle_start_timestamp < env.block.time.seconds() {
        return Err(ContractError::InvalidInitialCycleStartTimestamp);
    }

    let protocol_fee_rate =
        validate_protocol_fee_rate(msg.protocol_fee_rate.unwrap_or(default_protocol_fee_rate()?))?;
    let burn_address = deps.api.addr_validate(&msg.burn_address)?.to_string();
    let protocol_fee_address = deps.api.addr_validate(&msg.protocol_fee_address)?.to_string();

    let config = Config {
        owner: owner.clone(),
        burn_address,
        protocol_fee_address,
        protocol_fee_rate,
        token_code_id: msg.token_code_id,
        token_address: None,
        burn_denom: msg
            .burn_denom
            .unwrap_or_else(|| DEFAULT_BURN_DENOM.to_string()),
        cycle_duration_seconds,
    };
    CONFIG.save(deps.storage, &config)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = GlobalState {
        current_cycle_id: 1,
        current_cycle_start: initial_cycle_start_timestamp,
        active_cycle_burned: Uint128::zero(),
        emitted_cycle_count: 0,
        total_supply: Uint128::zero(),
        total_staked: Uint128::zero(),
        pending_reward_pool: Uint128::zero(),
        reward_index: Decimal::zero(),
    };
    GLOBAL_STATE.save(deps.storage, &state)?;

    let token_label = msg
        .token_label
        .unwrap_or_else(|| format!("isotropy-token-{}", env.contract.address));
    let instantiate_token = WasmMsg::Instantiate {
        admin: Some(owner.to_string()),
        code_id: config.token_code_id,
        msg: to_json_binary(&controller_instantiate_msg(env.contract.address.to_string()))?,
        funds: vec![],
        label: token_label,
    };

    Ok(Response::new()
        .add_submessage(cosmwasm_std::SubMsg::reply_on_success(
            instantiate_token,
            TOKEN_INSTANTIATE_REPLY_ID,
        ))
        .add_attributes([
            attr("action", "instantiate"),
            attr("owner", owner.as_str()),
            attr("burn_denom", config.burn_denom),
            attr("protocol_fee_rate", config.protocol_fee_rate.to_string()),
            attr("cycle_duration_seconds", cycle_duration_seconds.to_string()),
            attr(
                "initial_cycle_start_timestamp",
                initial_cycle_start_timestamp.to_string(),
            ),
            attr("token_code_id", config.token_code_id.to_string()),
        ]))
}

#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    if reply.id != TOKEN_INSTANTIATE_REPLY_ID {
        return Err(ContractError::Std(StdError::generic_err("unknown reply id")));
    }

    let parsed = parse_reply_instantiate_data(reply)?;
    let token_address = deps.api.addr_validate(&parsed.contract_address)?;

    CONFIG.update(deps.storage, |mut config| -> Result<_, ContractError> {
        if config.token_address.is_some() {
            return Err(ContractError::TokenAlreadyConfigured);
        }
        config.token_address = Some(token_address.clone());
        Ok(config)
    })?;

    Ok(Response::new().add_attributes([
        attr("action", "token_reply"),
        attr("token_address", token_address.as_str()),
    ]))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Burn { batches } => execute_burn(deps, env, info, batches),
        ExecuteMsg::AdvanceCycle {} => execute_advance_cycle(deps, env, info),
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::Stake { amount } => execute_stake(deps, env, info, amount),
        ExecuteMsg::Unstake { amount } => execute_unstake(deps, env, info, amount),
        ExecuteMsg::ClaimRewards {} => execute_claim_rewards(deps, env, info),
        ExecuteMsg::UpdateConfig {
            owner,
            burn_address,
            protocol_fee_address,
            protocol_fee_rate,
            cycle_duration_seconds,
        } => execute_update_config(
            deps,
            info,
            owner,
            burn_address,
            protocol_fee_address,
            protocol_fee_rate,
            cycle_duration_seconds,
        ),
    }
}

fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    batches: u32,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = GLOBAL_STATE.load(deps.storage)?;
    if env.block.time.seconds() < state.current_cycle_start {
        return Err(ContractError::CycleNotStarted);
    }
    let advance = maybe_advance_cycles(
        deps.storage,
        &env,
        &env.contract.address,
        &config,
        &mut state,
    )?;

    let quote = burn_quote(batches, config.protocol_fee_rate)?;
    let sent = must_pay(&info, &config.burn_denom);
    if sent != quote.total_amount {
        return Err(ContractError::InvalidPayment);
    }

    let mut position = settle_position(deps.storage, &state, &info.sender)?;
    position.reward_index = state.reward_index;
    position.next_cycle_to_process = position.next_cycle_to_process.max(1);
    POSITIONS.save(deps.storage, &info.sender, &position)?;

    let current_burn = CYCLE_BURNS
        .may_load(deps.storage, (state.current_cycle_id, &info.sender))?
        .unwrap_or_default();
    let updated_burn = current_burn.checked_add(quote.burn_amount)?;
    CYCLE_BURNS.save(
        deps.storage,
        (state.current_cycle_id, &info.sender),
        &updated_burn,
    )?;

    state.active_cycle_burned = state.active_cycle_burned.checked_add(quote.burn_amount)?;
    state.pending_reward_pool = state
        .pending_reward_pool
        .checked_add(quote.allocation_fee_amount)?;
    GLOBAL_STATE.save(deps.storage, &state)?;

    let mut messages = advance.messages;
    messages.push(
        BankMsg::Send {
            to_address: config.burn_address,
            amount: vec![Coin::new(
                quote.burn_amount.u128(),
                config.burn_denom.clone(),
            )],
        }
        .into(),
    );
    messages.push(
        BankMsg::Send {
            to_address: config.protocol_fee_address,
            amount: vec![Coin::new(
                quote.protocol_fee_amount.u128(),
                config.burn_denom.clone(),
            )],
        }
        .into(),
    );

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes([
            attr("action", "burn"),
            attr("sender", info.sender.as_str()),
            attr("cycle_id", state.current_cycle_id.to_string()),
            attr("batches", batches.to_string()),
            attr("burn_amount", quote.burn_amount.to_string()),
            attr("protocol_fee_amount", quote.protocol_fee_amount.to_string()),
            attr("allocation_fee_amount", quote.allocation_fee_amount.to_string()),
            attr("total_amount", quote.total_amount.to_string()),
            attr("advanced_cycles", advance.advanced_cycles.to_string()),
        ]))
}

fn execute_advance_cycle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    ensure_no_funds(&info)?;

    let config = CONFIG.load(deps.storage)?;
    let mut state = GLOBAL_STATE.load(deps.storage)?;
    let advance = maybe_advance_cycles(
        deps.storage,
        &env,
        &env.contract.address,
        &config,
        &mut state,
    )?;

    if advance.advanced_cycles == 0 {
        return Err(ContractError::CycleNotReady);
    }

    GLOBAL_STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_messages(advance.messages)
        .add_attributes([
            attr("action", "advance_cycle"),
            attr("advanced_cycles", advance.advanced_cycles.to_string()),
            attr("current_cycle_id", state.current_cycle_id.to_string()),
        ]))
}

fn execute_receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let token_address = token_address(&config)?;
    if info.sender != token_address {
        return Err(ContractError::InvalidTokenSender);
    }

    let hook: Cw20HookMsg =
        cosmwasm_std::from_json(msg.msg).map_err(|_| ContractError::InvalidCw20HookMsg)?;
    match hook {
        Cw20HookMsg::Stake {} => stake_tokens(deps, env, msg.sender, msg.amount, None),
    }
}

fn execute_stake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    ensure_no_funds(&info)?;

    let transfer_msg = {
        let config = CONFIG.load(deps.storage)?;
        let token_address = token_address(&config)?;
        Some(
            WasmMsg::Execute {
                contract_addr: token_address.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: info.sender.to_string(),
                    recipient: env.contract.address.to_string(),
                    amount,
                })?,
                funds: vec![],
            }
            .into(),
        )
    };

    stake_tokens(deps, env, info.sender.to_string(), amount, transfer_msg)
}

fn stake_tokens(
    deps: DepsMut,
    env: Env,
    sender: String,
    amount: Uint128,
    transfer_msg: Option<CosmosMsg>,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(&sender)?;
    let config = CONFIG.load(deps.storage)?;
    let mut state = GLOBAL_STATE.load(deps.storage)?;
    let advance = maybe_advance_cycles(
        deps.storage,
        &env,
        &env.contract.address,
        &config,
        &mut state,
    )?;

    let mut position = settle_position(deps.storage, &state, &sender)?;
    position.staked_balance = position.staked_balance.checked_add(amount)?;
    position.reward_index = state.reward_index;
    POSITIONS.save(deps.storage, &sender, &position)?;

    state.total_staked = state.total_staked.checked_add(amount)?;
    GLOBAL_STATE.save(deps.storage, &state)?;

    let mut messages = advance.messages;
    if let Some(transfer_msg) = transfer_msg {
        messages.push(transfer_msg);
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes([
            attr("action", "stake"),
            attr("sender", sender.as_str()),
            attr("amount", amount.to_string()),
            attr("advanced_cycles", advance.advanced_cycles.to_string()),
        ]))
}

fn execute_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    ensure_no_funds(&info)?;

    let config = CONFIG.load(deps.storage)?;
    let token_address = token_address(&config)?;
    let mut state = GLOBAL_STATE.load(deps.storage)?;
    let advance = maybe_advance_cycles(
        deps.storage,
        &env,
        &env.contract.address,
        &config,
        &mut state,
    )?;

    let mut position = settle_position(deps.storage, &state, &info.sender)?;
    if position.staked_balance < amount {
        return Err(ContractError::InsufficientStakedBalance);
    }

    position.staked_balance = position.staked_balance.checked_sub(amount)?;
    position.reward_index = state.reward_index;
    POSITIONS.save(deps.storage, &info.sender, &position)?;

    state.total_staked = state.total_staked.checked_sub(amount)?;
    GLOBAL_STATE.save(deps.storage, &state)?;

    let mut messages = advance.messages;
    messages.push(
        WasmMsg::Execute {
            contract_addr: token_address.to_string(),
            msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                recipient: info.sender.to_string(),
                amount,
            })?,
            funds: vec![],
        }
        .into(),
    );

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes([
            attr("action", "unstake"),
            attr("sender", info.sender.as_str()),
            attr("amount", amount.to_string()),
            attr("advanced_cycles", advance.advanced_cycles.to_string()),
        ]))
}

fn execute_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    ensure_no_funds(&info)?;

    let config = CONFIG.load(deps.storage)?;
    let mut state = GLOBAL_STATE.load(deps.storage)?;
    let advance = maybe_advance_cycles(
        deps.storage,
        &env,
        &env.contract.address,
        &config,
        &mut state,
    )?;

    let mut position = settle_position(deps.storage, &state, &info.sender)?;
    let reward_amount = position.pending_lunc_rewards;
    if reward_amount.is_zero() {
        return Err(ContractError::NoRewards);
    }

    position.pending_lunc_rewards = Uint128::zero();
    position.reward_index = state.reward_index;
    POSITIONS.save(deps.storage, &info.sender, &position)?;
    GLOBAL_STATE.save(deps.storage, &state)?;

    let mut messages = advance.messages;
    messages.push(
        BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin::new(reward_amount.u128(), config.burn_denom)],
        }
        .into(),
    );

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes([
            attr("action", "claim_rewards"),
            attr("sender", info.sender.as_str()),
            attr("amount", reward_amount.to_string()),
            attr("advanced_cycles", advance.advanced_cycles.to_string()),
        ]))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    burn_address: Option<String>,
    protocol_fee_address: Option<String>,
    protocol_fee_rate: Option<Decimal>,
    cycle_duration_seconds: Option<u64>,
) -> Result<Response, ContractError> {
    ensure_no_funds(&info)?;

    CONFIG.update(deps.storage, |mut config| -> Result<_, ContractError> {
        if info.sender != config.owner {
            return Err(ContractError::Unauthorized);
        }

        if let Some(owner) = owner {
            config.owner = deps.api.addr_validate(&owner)?;
        }
        if let Some(burn_address) = burn_address {
            config.burn_address = deps.api.addr_validate(&burn_address)?.to_string();
        }
        if let Some(protocol_fee_address) = protocol_fee_address {
            config.protocol_fee_address = deps.api.addr_validate(&protocol_fee_address)?.to_string();
        }
        if let Some(protocol_fee_rate) = protocol_fee_rate {
            config.protocol_fee_rate = validate_protocol_fee_rate(protocol_fee_rate)?;
        }
        if let Some(cycle_duration_seconds) = cycle_duration_seconds {
            if cycle_duration_seconds == 0 {
                return Err(ContractError::InvalidCycleDuration);
            }
            config.cycle_duration_seconds = cycle_duration_seconds;
        }

        Ok(config)
    })?;

    let updated = CONFIG.load(deps.storage)?;
    Ok(Response::new().add_attributes([
        attr("action", "update_config"),
        attr("protocol_fee_rate", updated.protocol_fee_rate.to_string()),
    ]))
}

#[entry_point]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let previous = get_contract_version(deps.storage).ok();
    let config = CONFIG.load(deps.storage)?;
    let mut state = GLOBAL_STATE.load(deps.storage)?;
    validate_protocol_fee_rate(config.protocol_fee_rate)?;
    state.emitted_cycle_count = count_historical_emitted_cycles(deps.storage)?;
    GLOBAL_STATE.save(deps.storage, &state)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut response = Response::new().add_attributes([
        attr("action", "migrate"),
        attr("contract_name", CONTRACT_NAME),
        attr("contract_version", CONTRACT_VERSION),
        attr("emitted_cycle_count", state.emitted_cycle_count.to_string()),
    ]);

    if let Some(previous) = previous {
        response = response.add_attributes([
            attr("previous_contract_name", previous.contract),
            attr("previous_contract_version", previous.version),
        ]);
    }

    Ok(response)
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::GlobalState {} => to_json_binary(&query_global_state(deps)?),
        QueryMsg::BurnQuote { batches } => to_json_binary(&query_burn_quote(deps, batches)?),
        QueryMsg::Position { address } => to_json_binary(&query_position(deps, address)?),
        QueryMsg::Cycle { cycle_id } => to_json_binary(&query_cycle(deps, cycle_id)?),
        QueryMsg::CurrentCycle {} => to_json_binary(&query_current_cycle(deps)?),
        QueryMsg::EmissionPoint { day } => to_json_binary(&EmissionPointResponse {
            day,
            amount: emission_for_cycle(day)?,
        }),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: config.owner.to_string(),
        burn_address: config.burn_address,
        protocol_fee_address: config.protocol_fee_address,
        protocol_fee_rate: config.protocol_fee_rate,
        token_address: config.token_address.map(|addr| addr.to_string()),
        token_code_id: config.token_code_id,
        burn_denom: config.burn_denom,
        cycle_duration_seconds: config.cycle_duration_seconds,
        token_symbol: TOKEN_SYMBOL.to_string(),
        token_decimals: TOKEN_DECIMALS,
        daily_decay_rate: daily_decay_rate()?,
    })
}

fn query_burn_quote(deps: Deps, batches: u32) -> StdResult<BurnQuoteResponse> {
    let config = CONFIG.load(deps.storage)?;
    burn_quote(batches, config.protocol_fee_rate)
}

fn query_global_state(deps: Deps) -> StdResult<GlobalStateResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = GLOBAL_STATE.load(deps.storage)?;
    Ok(GlobalStateResponse {
        current_cycle_id: state.current_cycle_id,
        current_cycle_start: state.current_cycle_start,
        current_cycle_end: state.current_cycle_start + config.cycle_duration_seconds,
        total_supply: state.total_supply,
        total_staked: state.total_staked,
        active_cycle_burned: state.active_cycle_burned,
        pending_reward_pool: state.pending_reward_pool,
        reward_index: state.reward_index,
    })
}

fn query_position(deps: Deps, address: String) -> StdResult<PositionResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = GLOBAL_STATE.load(deps.storage)?;
    let address = deps.api.addr_validate(&address)?;
    let position = settle_position_readonly(deps, &state, &address)
        .map_err(|err| StdError::generic_err(err.to_string()))?;
    let current_cycle_burned = CYCLE_BURNS
        .may_load(deps.storage, (state.current_cycle_id, &address))?
        .unwrap_or_default();

    Ok(PositionResponse {
        address: address.to_string(),
        liquid_balance: query_wallet_balance(deps, &config, &address)?,
        staked_balance: position.staked_balance,
        pending_lunc_rewards: position.pending_lunc_rewards,
        current_cycle_burned,
        next_cycle_to_process: position.next_cycle_to_process,
    })
}

fn query_cycle(deps: Deps, cycle_id: u64) -> StdResult<CycleResponse> {
    let cycle = CYCLE_SUMMARIES.load(deps.storage, cycle_id)?;
    Ok(CycleResponse {
        cycle_id: cycle.cycle_id,
        start_time: cycle.start_time,
        end_time: cycle.end_time,
        total_burned: cycle.total_burned,
        emission_amount: cycle.emission_amount,
        reward_pool: cycle.reward_pool,
        reward_index_before: cycle.reward_index_before,
        reward_index_after: cycle.reward_index_after,
    })
}

fn query_current_cycle(deps: Deps) -> StdResult<CurrentCycleResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = GLOBAL_STATE.load(deps.storage)?;
    Ok(CurrentCycleResponse {
        cycle_id: state.current_cycle_id,
        start_time: state.current_cycle_start,
        end_time: state.current_cycle_start + config.cycle_duration_seconds,
        total_burned: state.active_cycle_burned,
        projected_emission: next_emission_amount(&state)?,
        pending_reward_pool: state.pending_reward_pool,
    })
}

fn maybe_advance_cycles(
    storage: &mut dyn cosmwasm_std::Storage,
    env: &Env,
    controller_address: &Addr,
    config: &Config,
    state: &mut GlobalState,
) -> Result<AdvanceResult, ContractError> {
    let mut advanced_cycles = 0u64;
    let mut total_mint_amount = Uint128::zero();

    while env.block.time.seconds() >= state.current_cycle_start + config.cycle_duration_seconds {
        total_mint_amount =
            total_mint_amount.checked_add(finalize_current_cycle(storage, config, state)?)?;
        advanced_cycles += 1;
    }

    if advanced_cycles > 0 {
        GLOBAL_STATE.save(storage, state)?;
    }

    let mut messages = Vec::new();
    if !total_mint_amount.is_zero() {
        let token_address = token_address(config)?;
        messages.push(
            WasmMsg::Execute {
                contract_addr: token_address.to_string(),
                msg: to_json_binary(&Cw20ExecuteMsg::Mint {
                    recipient: controller_address.to_string(),
                    amount: total_mint_amount,
                })?,
                funds: vec![],
            }
            .into(),
        );
    }

    Ok(AdvanceResult {
        advanced_cycles,
        messages,
    })
}

fn finalize_current_cycle(
    storage: &mut dyn cosmwasm_std::Storage,
    config: &Config,
    state: &mut GlobalState,
) -> Result<Uint128, ContractError> {
    let cycle_id = state.current_cycle_id;
    let reward_index_before = state.reward_index;
    let emission_amount = if state.active_cycle_burned.is_zero() {
        Uint128::zero()
    } else {
        next_emission_amount(state)?
    };

    if !emission_amount.is_zero() {
        state.total_supply = state.total_supply.checked_add(emission_amount)?;
        state.total_staked = state.total_staked.checked_add(emission_amount)?;
        state.emitted_cycle_count += 1;
    }

    let mut reward_index_after = reward_index_before;
    let mut distributed_reward_pool = Uint128::zero();
    if !state.pending_reward_pool.is_zero() && !state.total_staked.is_zero() {
        distributed_reward_pool = state.pending_reward_pool;
        let reward_delta = Decimal::from_ratio(distributed_reward_pool, state.total_staked);
        reward_index_after = reward_index_after + reward_delta;
        state.reward_index = reward_index_after;
        state.pending_reward_pool = Uint128::zero();
    }

    let end_time = state.current_cycle_start + config.cycle_duration_seconds;
    let summary = CycleSummary {
        cycle_id,
        start_time: state.current_cycle_start,
        end_time,
        total_burned: state.active_cycle_burned,
        emission_amount,
        reward_pool: distributed_reward_pool,
        reward_index_before,
        reward_index_after,
    };
    CYCLE_SUMMARIES.save(storage, cycle_id, &summary)?;

    state.current_cycle_id += 1;
    state.current_cycle_start = end_time;
    state.active_cycle_burned = Uint128::zero();

    Ok(emission_amount)
}

fn next_emission_amount(state: &GlobalState) -> StdResult<Uint128> {
    emission_for_cycle(state.emitted_cycle_count.saturating_add(1))
}

fn count_historical_emitted_cycles(storage: &dyn cosmwasm_std::Storage) -> StdResult<u64> {
    CYCLE_SUMMARIES
        .range(storage, None, None, Order::Ascending)
        .try_fold(0u64, |count, item| {
            let (_, cycle) = item?;
            Ok(if cycle.emission_amount.is_zero() {
                count
            } else {
                count + 1
            })
        })
}

fn settle_position(
    storage: &dyn cosmwasm_std::Storage,
    state: &GlobalState,
    address: &Addr,
) -> Result<UserPosition, ContractError> {
    let mut position = POSITIONS.may_load(storage, address)?.unwrap_or_default();
    reconcile_position(storage, state, address, &mut position)?;
    Ok(position)
}

fn settle_position_readonly(
    deps: Deps,
    state: &GlobalState,
    address: &Addr,
) -> Result<UserPosition, ContractError> {
    let mut position = POSITIONS.may_load(deps.storage, address)?.unwrap_or_default();
    reconcile_position(deps.storage, state, address, &mut position)?;
    Ok(position)
}

fn reconcile_position(
    storage: &dyn cosmwasm_std::Storage,
    state: &GlobalState,
    address: &Addr,
    position: &mut UserPosition,
) -> Result<(), ContractError> {
    let mut cursor = position.next_cycle_to_process.max(1);

    while cursor < state.current_cycle_id {
        let cycle = CYCLE_SUMMARIES.load(storage, cursor)?;

        sync_rewards(position, cycle.reward_index_before)?;

        let burned = CYCLE_BURNS
            .may_load(storage, (cursor, address))?
            .unwrap_or_default();
        if !burned.is_zero() && !cycle.total_burned.is_zero() {
            let emission_share = cycle
                .emission_amount
                .multiply_ratio(burned.u128(), cycle.total_burned.u128());
            position.staked_balance = position.staked_balance.checked_add(emission_share)?;
        }

        sync_rewards(position, cycle.reward_index_after)?;
        position.next_cycle_to_process = cursor + 1;
        cursor += 1;
    }

    sync_rewards(position, state.reward_index)?;
    Ok(())
}

fn sync_rewards(position: &mut UserPosition, next_reward_index: Decimal) -> Result<(), ContractError> {
    if position.staked_balance.is_zero() {
        position.reward_index = next_reward_index;
        return Ok(());
    }

    let reward_delta = next_reward_index
        .checked_sub(position.reward_index)
        .map_err(StdError::overflow)?;
    if reward_delta.is_zero() {
        position.reward_index = next_reward_index;
        return Ok(());
    }

    let accrued = position.staked_balance.multiply_ratio(
        reward_delta.atomics().u128(),
        DECIMAL_FRACTIONAL,
    );
    position.pending_lunc_rewards = position.pending_lunc_rewards.checked_add(accrued)?;
    position.reward_index = next_reward_index;
    Ok(())
}

fn query_wallet_balance(deps: Deps, config: &Config, address: &Addr) -> StdResult<Uint128> {
    match &config.token_address {
        Some(token_address) => {
            let response: Cw20BalanceResponse = deps.querier.query_wasm_smart(
                token_address,
                &Cw20QueryMsg::Balance {
                    address: address.to_string(),
                },
            )?;
            Ok(response.balance)
        }
        None => Ok(Uint128::zero()),
    }
}

fn token_address(config: &Config) -> Result<Addr, ContractError> {
    config
        .token_address
        .clone()
        .ok_or(ContractError::TokenNotConfigured)
}

fn ensure_no_funds(info: &MessageInfo) -> Result<(), ContractError> {
    if info.funds.is_empty() {
        return Ok(());
    }

    Err(ContractError::UnexpectedFunds)
}

fn burn_quote(batches: u32, protocol_fee_rate: Decimal) -> StdResult<BurnQuoteResponse> {
    if !(MIN_BATCHES..=MAX_BATCHES).contains(&batches) {
        return Err(StdError::generic_err("invalid batches"));
    }

    let burn_amount = Uint128::new(BATCH_SIZE_LUNC * batches as u128);
    let allocation_fee_rate = allocation_fee_rate(batches)?;
    let protocol_fee_amount = ratio_amount(burn_amount, protocol_fee_rate);
    let allocation_fee_amount = ratio_amount(burn_amount, allocation_fee_rate);
    let total_amount = burn_amount
        .checked_add(protocol_fee_amount)?
        .checked_add(allocation_fee_amount)?;

    Ok(BurnQuoteResponse {
        batches,
        burn_amount,
        protocol_fee_amount,
        allocation_fee_amount,
        total_amount,
        allocation_fee_rate,
    })
}

fn must_pay(info: &MessageInfo, denom: &str) -> Uint128 {
    info.funds
        .iter()
        .find(|coin| coin.denom == denom)
        .map(|coin| coin.amount)
        .unwrap_or_default()
}

fn ratio_amount(amount: Uint128, ratio: Decimal) -> Uint128 {
    amount.multiply_ratio(ratio.atomics().u128(), DECIMAL_FRACTIONAL)
}

fn default_protocol_fee_rate() -> StdResult<Decimal> {
    Decimal::from_str(DEFAULT_PROTOCOL_FEE_RATE_STR)
        .map_err(|err| StdError::generic_err(err.to_string()))
}

fn min_protocol_fee_rate() -> StdResult<Decimal> {
    Decimal::from_str(MIN_PROTOCOL_FEE_RATE_STR).map_err(|err| StdError::generic_err(err.to_string()))
}

fn validate_protocol_fee_rate(rate: Decimal) -> Result<Decimal, ContractError> {
    let min_rate = min_protocol_fee_rate()?;
    let max_rate = default_protocol_fee_rate()?;

    if rate < min_rate || rate > max_rate {
        return Err(ContractError::InvalidProtocolFeeRate);
    }

    Ok(rate)
}

fn allocation_fee_rate(batches: u32) -> StdResult<Decimal> {
    let max_rate =
        Decimal::from_str(ALLOCATION_FEE_MAX_STR).map_err(|err| StdError::generic_err(err.to_string()))?;
    let min_rate =
        Decimal::from_str(ALLOCATION_FEE_MIN_STR).map_err(|err| StdError::generic_err(err.to_string()))?;

    if batches <= MIN_BATCHES {
        return Ok(max_rate);
    }
    if batches >= MAX_BATCHES {
        return Ok(min_rate);
    }

    let span = max_rate - min_rate;
    let progress = Decimal::from_ratio((batches - 1) as u128, (MAX_BATCHES - 1) as u128);
    Ok(max_rate - (span * progress))
}

fn daily_decay_rate() -> StdResult<Decimal> {
    Decimal::from_str(DAILY_DECAY_RATE_STR).map_err(|err| StdError::generic_err(err.to_string()))
}

fn emission_for_cycle(day: u64) -> StdResult<Uint128> {
    if day == 0 || day > EMISSION_DAYS {
        return Ok(Uint128::zero());
    }

    let daily_decay = daily_decay_rate()?;
    let daily_multiplier = Decimal::one() - daily_decay;
    let mut factor = Decimal::one();

    for _ in 1..day {
        factor = factor * daily_multiplier;
    }

    Ok(Uint128::new(INITIAL_DAILY_EMISSION).multiply_ratio(
        factor.atomics().u128(),
        DECIMAL_FRACTIONAL,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{coin, Empty, Timestamp};
    use cw20::{AllowanceResponse, TokenInfoResponse};
    use cw_multi_test::{App, AppBuilder, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};

    fn configured_protocol_fee_rate() -> Decimal {
        default_protocol_fee_rate().unwrap()
    }

    fn burn_quote_default(batches: u32) -> BurnQuoteResponse {
        burn_quote(batches, configured_protocol_fee_rate()).unwrap()
    }

    fn controller_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(execute, instantiate, query)
            .with_reply(reply)
            .with_migrate(migrate);
        Box::new(contract)
    }

    fn token_contract() -> Box<dyn Contract<Empty>> {
        Box::new(ContractWrapper::new(
            isotropy_token::execute,
            isotropy_token::instantiate,
            isotropy_token::query,
        ))
    }

    fn mock_app() -> App {
        AppBuilder::new().build(|_, _, _| {})
    }

    fn mint_native(app: &mut App, recipient: &str, amount: u128) {
        app.sudo(SudoMsg::Bank(BankSudo::Mint {
            to_address: recipient.to_string(),
            amount: vec![coin(amount, "uluna")],
        }))
        .unwrap();
    }

    fn instantiate_default(app: &mut App) -> Addr {
        let controller_code_id = app.store_code(controller_contract());
        let token_code_id = app.store_code(token_contract());

        app.instantiate_contract(
            controller_code_id,
            Addr::unchecked("owner"),
            &InstantiateMsg {
                owner: None,
                burn_address: "burn-placeholder".to_string(),
                protocol_fee_address: "fee-placeholder".to_string(),
                protocol_fee_rate: None,
                token_code_id,
                token_label: Some("isotropy-token-testnet".to_string()),
                burn_denom: Some("uluna".to_string()),
                cycle_duration_seconds: Some(100),
                initial_cycle_start_timestamp: None,
            },
            &[],
            "isotropy-controller",
            None,
        )
        .unwrap()
    }

    fn query_config_response(app: &App, controller: &Addr) -> ConfigResponse {
        app.wrap()
            .query_wasm_smart(controller, &QueryMsg::Config {})
            .unwrap()
    }

    fn query_position_response(app: &App, controller: &Addr, address: &str) -> PositionResponse {
        app.wrap()
            .query_wasm_smart(
                controller,
                &QueryMsg::Position {
                    address: address.to_string(),
                },
            )
            .unwrap()
    }

    fn query_token_balance(app: &App, token: &Addr, address: &str) -> Uint128 {
        let response: Cw20BalanceResponse = app
            .wrap()
            .query_wasm_smart(
                token,
                &Cw20QueryMsg::Balance {
                    address: address.to_string(),
                },
            )
            .unwrap();
        response.balance
    }

    fn execute_burn_default(
        app: &mut App,
        controller: &Addr,
        sender: &str,
        batches: u32,
    ) -> BurnQuoteResponse {
        let quote: BurnQuoteResponse = app
            .wrap()
            .query_wasm_smart(controller, &QueryMsg::BurnQuote { batches })
            .unwrap();
        mint_native(app, sender, quote.total_amount.u128());
        app.execute_contract(
            Addr::unchecked(sender),
            controller.clone(),
            &ExecuteMsg::Burn { batches },
            &[coin(quote.total_amount.u128(), "uluna")],
        )
        .unwrap();
        quote
    }

    fn advance_cycle(app: &mut App, controller: &Addr) {
        app.update_block(|block| {
            block.time = block.time.plus_seconds(100);
        });
        app.execute_contract(
            Addr::unchecked("keeper"),
            controller.clone(),
            &ExecuteMsg::AdvanceCycle {},
            &[],
        )
        .unwrap();
    }

    #[test]
    fn missed_empty_cycles_still_advance_without_minting() {
        let mut deps = mock_dependencies();
        let controller_address = Addr::unchecked("controller");
        let cycle_duration_seconds = 100u64;

        CONFIG
            .save(
                deps.as_mut().storage,
                &Config {
                    owner: Addr::unchecked("owner"),
                    burn_address: "burn-placeholder".to_string(),
                    protocol_fee_address: "fee-placeholder".to_string(),
                    protocol_fee_rate: configured_protocol_fee_rate(),
                    token_code_id: 1,
                    token_address: Some(Addr::unchecked("token")),
                    burn_denom: "uluna".to_string(),
                    cycle_duration_seconds,
                },
            )
            .unwrap();
        GLOBAL_STATE
            .save(
                deps.as_mut().storage,
                &GlobalState {
                    current_cycle_id: 1,
                    current_cycle_start: 1_000,
                    active_cycle_burned: Uint128::zero(),
                    emitted_cycle_count: 0,
                    total_supply: Uint128::zero(),
                    total_staked: Uint128::zero(),
                    pending_reward_pool: Uint128::zero(),
                    reward_index: Decimal::zero(),
                },
            )
            .unwrap();

        let config = CONFIG.load(deps.as_ref().storage).unwrap();
        let mut state = GLOBAL_STATE.load(deps.as_ref().storage).unwrap();
        let mut env = mock_env();
        env.contract.address = controller_address.clone();
        env.block.time = Timestamp::from_seconds(1_000 + (cycle_duration_seconds * 3));

        let advance = maybe_advance_cycles(
            deps.as_mut().storage,
            &env,
            &controller_address,
            &config,
            &mut state,
        )
        .unwrap();

        assert_eq!(advance.advanced_cycles, 3);
        assert!(advance.messages.is_empty());

        let saved_state = GLOBAL_STATE.load(deps.as_ref().storage).unwrap();
        assert_eq!(saved_state.current_cycle_id, 4);
        assert_eq!(saved_state.current_cycle_start, 1_000 + (cycle_duration_seconds * 3));
        assert_eq!(saved_state.emitted_cycle_count, 0);
        assert_eq!(saved_state.total_supply, Uint128::zero());
        assert_eq!(saved_state.total_staked, Uint128::zero());
    }

    #[test]
    fn zero_burn_cycles_do_not_consume_emission_schedule() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);
        let token = Addr::unchecked(query_config_response(&app, &controller).token_address.unwrap());
        let first_emission = emission_for_cycle(1).unwrap();
        let second_emission = emission_for_cycle(2).unwrap();

        advance_cycle(&mut app, &controller);

        let after_empty_cycle: CurrentCycleResponse = app
            .wrap()
            .query_wasm_smart(&controller, &QueryMsg::CurrentCycle {})
            .unwrap();
        assert_eq!(after_empty_cycle.cycle_id, 2);
        assert_eq!(after_empty_cycle.projected_emission, first_emission);

        let empty_cycle_one: CycleResponse = app
            .wrap()
            .query_wasm_smart(&controller, &QueryMsg::Cycle { cycle_id: 1 })
            .unwrap();
        assert_eq!(empty_cycle_one.total_burned, Uint128::zero());
        assert_eq!(empty_cycle_one.emission_amount, Uint128::zero());

        execute_burn_default(&mut app, &controller, "alice", 1);
        advance_cycle(&mut app, &controller);

        let alice = query_position_response(&app, &controller, "alice");
        assert_eq!(alice.staked_balance, first_emission);

        let supply_after_first_productive_cycle: TokenInfoResponse = app
            .wrap()
            .query_wasm_smart(&token, &Cw20QueryMsg::TokenInfo {})
            .unwrap();
        assert_eq!(supply_after_first_productive_cycle.total_supply, first_emission);

        advance_cycle(&mut app, &controller);

        let after_second_empty_cycle: CurrentCycleResponse = app
            .wrap()
            .query_wasm_smart(&controller, &QueryMsg::CurrentCycle {})
            .unwrap();
        assert_eq!(after_second_empty_cycle.cycle_id, 4);
        assert_eq!(after_second_empty_cycle.projected_emission, second_emission);

        let empty_cycle_three: CycleResponse = app
            .wrap()
            .query_wasm_smart(&controller, &QueryMsg::Cycle { cycle_id: 3 })
            .unwrap();
        assert_eq!(empty_cycle_three.total_burned, Uint128::zero());
        assert_eq!(empty_cycle_three.emission_amount, Uint128::zero());

        execute_burn_default(&mut app, &controller, "bob", 1);
        advance_cycle(&mut app, &controller);

        let bob = query_position_response(&app, &controller, "bob");
        assert_eq!(bob.staked_balance, second_emission);

        let final_supply: TokenInfoResponse = app
            .wrap()
            .query_wasm_smart(&token, &Cw20QueryMsg::TokenInfo {})
            .unwrap();
        assert_eq!(
            final_supply.total_supply,
            first_emission.checked_add(second_emission).unwrap()
        );
    }

    #[test]
    fn emission_curve_matches_key_targets() {
        assert_eq!(emission_for_cycle(1).unwrap(), Uint128::new(1_332_820_936));
        assert_eq!(emission_for_cycle(52_560).unwrap(), Uint128::new(932_067_245));
        assert_eq!(emission_for_cycle(105_120).unwrap(), Uint128::new(651_808_067));
        assert_eq!(emission_for_cycle(105_121).unwrap(), Uint128::zero());
    }

    #[test]
    fn quote_gradient_is_linear() {
        let one_batch = burn_quote_default(1);
        let mid_batch = burn_quote_default(5_000);
        let max_batch = burn_quote_default(10_000);

        assert_eq!(
            one_batch.allocation_fee_rate,
            Decimal::from_str(ALLOCATION_FEE_MAX_STR).unwrap()
        );
        assert_eq!(
            max_batch.allocation_fee_rate,
            Decimal::from_str(ALLOCATION_FEE_MIN_STR).unwrap()
        );
        assert!(mid_batch.allocation_fee_rate < one_batch.allocation_fee_rate);
        assert!(mid_batch.allocation_fee_rate > max_batch.allocation_fee_rate);
    }

    #[test]
    fn instantiate_creates_token_and_sets_zero_total_supply() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);
        let config = query_config_response(&app, &controller);
        let token = Addr::unchecked(config.token_address.unwrap());

        let info: TokenInfoResponse = app
            .wrap()
            .query_wasm_smart(&token, &Cw20QueryMsg::TokenInfo {})
            .unwrap();

        assert_eq!(config.token_code_id > 0, true);
        assert_eq!(info.total_supply, Uint128::zero());
        assert_eq!(info.symbol, TOKEN_SYMBOL);
    }

    #[test]
    fn burn_and_cycle_advance_auto_stake_emission_into_real_cw20_supply() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);
        let token = Addr::unchecked(query_config_response(&app, &controller).token_address.unwrap());
        let first_cycle_emission = emission_for_cycle(1).unwrap();

        execute_burn_default(&mut app, &controller, "alice", 2);
        advance_cycle(&mut app, &controller);

        let position = query_position_response(&app, &controller, "alice");
        assert_eq!(position.current_cycle_burned, Uint128::zero());
        assert_eq!(position.staked_balance, first_cycle_emission);
        assert_eq!(position.liquid_balance, Uint128::zero());
        assert_eq!(query_token_balance(&app, &token, controller.as_str()), first_cycle_emission);
    }

    #[test]
    fn claim_rewards_transfers_native_lunc_to_staker() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);

        let quote = execute_burn_default(&mut app, &controller, "alice", 1);
        advance_cycle(&mut app, &controller);

        app.execute_contract(
            Addr::unchecked("alice"),
            controller.clone(),
            &ExecuteMsg::ClaimRewards {},
            &[],
        )
        .unwrap();

        let balance = app.wrap().query_balance("alice", "uluna").unwrap();
        let position = query_position_response(&app, &controller, "alice");

        assert!(balance.amount <= quote.allocation_fee_amount);
        assert!(
            quote
                .allocation_fee_amount
                .checked_sub(balance.amount)
                .unwrap()
                <= Uint128::new(1)
        );
        assert_eq!(position.pending_lunc_rewards, Uint128::zero());
    }

    #[test]
    fn unstake_then_restake_supports_allowance_and_send_flows() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);
        let token = Addr::unchecked(query_config_response(&app, &controller).token_address.unwrap());

        execute_burn_default(&mut app, &controller, "alice", 1);
        advance_cycle(&mut app, &controller);

        app.execute_contract(
            Addr::unchecked("alice"),
            controller.clone(),
            &ExecuteMsg::Unstake {
                amount: Uint128::new(300_000_000),
            },
            &[],
        )
        .unwrap();

        app.execute_contract(
            Addr::unchecked("alice"),
            token.clone(),
            &Cw20ExecuteMsg::IncreaseAllowance {
                spender: controller.to_string(),
                amount: Uint128::new(100_000_000),
                expires: None,
            },
            &[],
        )
        .unwrap();
        app.execute_contract(
            Addr::unchecked("alice"),
            controller.clone(),
            &ExecuteMsg::Stake {
                amount: Uint128::new(100_000_000),
            },
            &[],
        )
        .unwrap();

        let allowance: AllowanceResponse = app
            .wrap()
            .query_wasm_smart(
                &token,
                &Cw20QueryMsg::Allowance {
                    owner: "alice".to_string(),
                    spender: controller.to_string(),
                },
            )
            .unwrap();
        assert_eq!(allowance.allowance, Uint128::zero());

        app.execute_contract(
            Addr::unchecked("alice"),
            token.clone(),
            &Cw20ExecuteMsg::Send {
                contract: controller.to_string(),
                amount: Uint128::new(50_000_000),
                msg: to_json_binary(&Cw20HookMsg::Stake {}).unwrap(),
            },
            &[],
        )
        .unwrap();

        let position = query_position_response(&app, &controller, "alice");
        assert_eq!(position.staked_balance, Uint128::new(1_182_820_936));
        assert_eq!(position.liquid_balance, Uint128::new(150_000_000));
        assert_eq!(
            query_token_balance(&app, &token, controller.as_str()),
            Uint128::new(1_182_820_936)
        );
    }

    #[test]
    fn rewards_split_across_existing_stakers_proportionally() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);

        let alice_burn = execute_burn_default(&mut app, &controller, "alice", 1);
        let bob_burn = execute_burn_default(&mut app, &controller, "bob", 2);
        advance_cycle(&mut app, &controller);

        let cycle_two_quote = execute_burn_default(&mut app, &controller, "carol", 1);
        advance_cycle(&mut app, &controller);

        let alice = query_position_response(&app, &controller, "alice");
        let bob = query_position_response(&app, &controller, "bob");
        let carol = query_position_response(&app, &controller, "carol");
        let total_rewards = alice
            .pending_lunc_rewards
            .checked_add(bob.pending_lunc_rewards)
            .unwrap()
            .checked_add(carol.pending_lunc_rewards)
            .unwrap();
        let expected_total_rewards = alice_burn
            .allocation_fee_amount
            .checked_add(bob_burn.allocation_fee_amount)
            .unwrap()
            .checked_add(cycle_two_quote.allocation_fee_amount)
            .unwrap();
        let rounding_dust = expected_total_rewards.checked_sub(total_rewards).unwrap();

        assert!(total_rewards <= expected_total_rewards);
        assert!(rounding_dust <= Uint128::new(100));
        assert!(bob.pending_lunc_rewards > alice.pending_lunc_rewards);
    }

    #[test]
    fn direct_receive_call_from_non_token_is_rejected() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);
        let err = app
            .execute_contract(
                Addr::unchecked("alice"),
                controller,
                &ExecuteMsg::Receive(Cw20ReceiveMsg {
                    sender: "alice".to_string(),
                    amount: Uint128::new(1),
                    msg: to_json_binary(&Cw20HookMsg::Stake {}).unwrap(),
                }),
                &[],
            )
            .unwrap_err();

        let err_text = format!("{err:?}");
        assert!(err_text.contains("unauthorized cw20 token sender"));
    }

    #[test]
    fn burn_is_rejected_before_scheduled_cycle_start() {
        let mut app = mock_app();
        let controller_code_id = app.store_code(controller_contract());
        let token_code_id = app.store_code(token_contract());
        let scheduled_start = app.block_info().time.seconds() + 300;

        let controller = app
            .instantiate_contract(
                controller_code_id,
                Addr::unchecked("owner"),
                &InstantiateMsg {
                    owner: None,
                    burn_address: "burn-placeholder".to_string(),
                    protocol_fee_address: "fee-placeholder".to_string(),
                    protocol_fee_rate: None,
                    token_code_id,
                    token_label: Some("isotropy-token-testnet".to_string()),
                    burn_denom: Some("uluna".to_string()),
                    cycle_duration_seconds: Some(100),
                    initial_cycle_start_timestamp: Some(scheduled_start),
                },
                &[],
                "isotropy-controller",
                None,
            )
            .unwrap();

        let current_cycle: CurrentCycleResponse = app
            .wrap()
            .query_wasm_smart(&controller, &QueryMsg::CurrentCycle {})
            .unwrap();
        assert_eq!(current_cycle.start_time, scheduled_start);

        let quote: BurnQuoteResponse = app
            .wrap()
            .query_wasm_smart(&controller, &QueryMsg::BurnQuote { batches: 1 })
            .unwrap();
        mint_native(&mut app, "alice", quote.total_amount.u128());

        let err = app
            .execute_contract(
                Addr::unchecked("alice"),
                controller,
                &ExecuteMsg::Burn { batches: 1 },
                &[coin(quote.total_amount.u128(), "uluna")],
            )
            .unwrap_err();

        let err_text = format!("{err:?}");
        assert!(err_text.contains("cycle has not started yet"));
    }

    #[test]
    fn owner_can_reduce_protocol_fee_rate() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);

        app.execute_contract(
            Addr::unchecked("owner"),
            controller.clone(),
            &ExecuteMsg::UpdateConfig {
                owner: None,
                burn_address: None,
                protocol_fee_address: None,
                protocol_fee_rate: Some(Decimal::from_str("0.025").unwrap()),
                cycle_duration_seconds: None,
            },
            &[],
        )
        .unwrap();

        let response = query_config_response(&app, &controller);
        assert_eq!(response.protocol_fee_rate, Decimal::from_str("0.025").unwrap());
    }

    #[test]
    fn protocol_fee_rate_cannot_drop_below_minimum() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);
        let err = app
            .execute_contract(
                Addr::unchecked("owner"),
                controller,
                &ExecuteMsg::UpdateConfig {
                    owner: None,
                    burn_address: None,
                    protocol_fee_address: None,
                    protocol_fee_rate: Some(Decimal::from_str("0.00009").unwrap()),
                    cycle_duration_seconds: None,
                },
                &[],
            )
            .unwrap_err();

        let err_text = format!("{err:?}");
        assert!(err_text.contains("invalid protocol fee rate"));
    }

    #[test]
    fn instantiate_rejects_invalid_distribution_addresses() {
        let mut app = mock_app();
        let controller_code_id = app.store_code(controller_contract());
        let token_code_id = app.store_code(token_contract());

        let err = app
            .instantiate_contract(
                controller_code_id,
                Addr::unchecked("owner"),
                &InstantiateMsg {
                    owner: None,
                    burn_address: "INVALID ADDRESS".to_string(),
                    protocol_fee_address: "fee-placeholder".to_string(),
                    protocol_fee_rate: None,
                    token_code_id,
                    token_label: Some("isotropy-token-testnet".to_string()),
                    burn_denom: Some("uluna".to_string()),
                    cycle_duration_seconds: Some(100),
                    initial_cycle_start_timestamp: None,
                },
                &[],
                "isotropy-controller",
                None,
            )
            .unwrap_err();

        assert!(!format!("{err:?}").is_empty());
    }

    #[test]
    fn update_config_rejects_invalid_distribution_addresses() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);

        let err = app
            .execute_contract(
                Addr::unchecked("owner"),
                controller,
                &ExecuteMsg::UpdateConfig {
                    owner: None,
                    burn_address: Some("INVALID ADDRESS".to_string()),
                    protocol_fee_address: None,
                    protocol_fee_rate: None,
                    cycle_duration_seconds: None,
                },
                &[],
            )
            .unwrap_err();

        assert!(!format!("{err:?}").is_empty());
    }

    #[test]
    fn token_supply_is_minted_only_as_cycles_advance() {
        let mut app = mock_app();
        let controller = instantiate_default(&mut app);
        let token = Addr::unchecked(query_config_response(&app, &controller).token_address.unwrap());

        let info_before: TokenInfoResponse = app
            .wrap()
            .query_wasm_smart(&token, &Cw20QueryMsg::TokenInfo {})
            .unwrap();
        assert_eq!(info_before.total_supply, Uint128::zero());

        execute_burn_default(&mut app, &controller, "alice", 1);
        advance_cycle(&mut app, &controller);

        let info_after: TokenInfoResponse = app
            .wrap()
            .query_wasm_smart(&token, &Cw20QueryMsg::TokenInfo {})
            .unwrap();
        assert_eq!(info_after.total_supply, Uint128::new(1_332_820_936));
    }
}

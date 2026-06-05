#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw20::MinterResponse;
use cw20_base::ContractError;
use cw20_base::contract::{
    execute as cw20_execute, instantiate as cw20_instantiate, query as cw20_query,
};
use cw20_base::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

pub use cw20_base::msg;

const TOKEN_NAME: &str = "Isotropy Burned LUNC";
const TOKEN_SYMBOL: &str = "isLUNC";
const TOKEN_DECIMALS: u8 = 6;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    cw20_instantiate(deps, env, info, msg)
}

pub fn controller_instantiate_msg(controller_address: String) -> InstantiateMsg {
    InstantiateMsg {
        name: TOKEN_NAME.to_string(),
        symbol: TOKEN_SYMBOL.to_string(),
        decimals: TOKEN_DECIMALS,
        initial_balances: vec![],
        mint: Some(MinterResponse {
            minter: controller_address,
            cap: None,
        }),
        marketing: None,
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    cw20_execute(deps, env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    cw20_query(deps, env, msg)
}

pub fn instantiate_for_controller(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    controller_address: String,
) -> Result<Response, ContractError> {
    cw20_instantiate(
        deps,
        env,
        info,
        controller_instantiate_msg(controller_address),
    )
}

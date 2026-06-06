use cosmwasm_std::{OverflowError, StdError};
use cw_utils::ParseReplyError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Overflow(#[from] OverflowError),

    #[error("{0}")]
    ParseReply(#[from] ParseReplyError),

    #[error("unauthorized")]
    Unauthorized,

    #[error("invalid batches")]
    InvalidBatches,

    #[error("invalid cycle duration")]
    InvalidCycleDuration,

    #[error("invalid initial cycle start timestamp")]
    InvalidInitialCycleStartTimestamp,

    #[error("delayed start can only be updated before cycle 1 begins")]
    DelayedStartLocked,

    #[error("invalid protocol fee rate")]
    InvalidProtocolFeeRate,

    #[error("contract funds do not match quote")]
    InvalidPayment,

    #[error("no rewards available")]
    NoRewards,

    #[error("insufficient staked balance")]
    InsufficientStakedBalance,

    #[error("cycle is not ready yet")]
    CycleNotReady,

    #[error("cycle has not started yet")]
    CycleNotStarted,

    #[error("cw20 token is not configured yet")]
    TokenNotConfigured,

    #[error("cw20 token is already configured")]
    TokenAlreadyConfigured,

    #[error("unauthorized cw20 token sender")]
    InvalidTokenSender,

    #[error("invalid cw20 hook message")]
    InvalidCw20HookMsg,

    #[error("native funds are not accepted for this message")]
    UnexpectedFunds,
}

use std::u64;

use cosmwasm_std::StdError;
use indexable_hooks::HookError;
use thiserror::Error;
use voting::threshold::ThresholdError;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    HookError(#[from] HookError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("{0}")]
    ThresholdError(#[from] ThresholdError),

    #[error("Suggested proposal expiration is larger than the maximum proposal duration")]
    InvalidExpiration {},

    #[error("No such proposal ({id})")]
    NoSuchProposal { id: u64 },

    #[error("Proposal is ({size}) bytes, must be <= ({max}) bytes")]
    ProposalTooLarge { size: u64, max: u64 },

    #[error("Proposal is not open ({id})")]
    NotOpen { id: u64 },

    #[error("Proposal is expired ({id})")]
    Expired { id: u64 },

    #[error("Not registered to vote (no voting power) at time of proposal creation.")]
    NotRegistered {},

    #[error("Already voted")]
    AlreadyVoted {},

    #[error("Proposal must be in 'passed' state to be executed.")]
    NotPassed {},

    #[error("Proposal is not expired.")]
    NotExpired {},

    #[error("Only rejected or expired proposals may be closed.")]
    WrongCloseStatus {},

    #[error("The DAO is currently inactive, you cannot create proposals.")]
    InactiveDao {},

    #[error("Must have at least three choices, one of them being none of the above.")]
    WrongNumberOfChoices {},

    #[error("Must have exactly one 'none of the above' option.")]
    NoneOption {},

    #[error("No vote weights found.")]
    NoVoteWeights {},
}

use cosmwasm_std::{Addr, Deps, StdResult, Uint128};
use cw_core;

use cw_core_interface::voting;
use cw_utils::Duration;

use crate::ContractError;

pub fn get_voting_power(
    deps: Deps,
    address: Addr,
    dao: Addr,
    height: Option<u64>,
) -> StdResult<Uint128> {
    let response: voting::VotingPowerAtHeightResponse = deps.querier.query_wasm_smart(
        dao,
        &voting::Query::VotingPowerAtHeight {
            address: address.to_string(),
            height,
        },
    )?;
    Ok(response.power)
}

pub fn get_total_power(deps: Deps, dao: Addr, height: Option<u64>) -> StdResult<Uint128> {
    let response: voting::TotalPowerAtHeightResponse = deps
        .querier
        .query_wasm_smart(dao, &voting::Query::TotalPowerAtHeight { height })?;
    Ok(response.power)
}

/// Validates that the min voting period is less than the max voting
/// period. Passes arguments through the function.
pub fn validate_voting_period(
    min: Option<Duration>,
    max: Duration,
) -> Result<(Option<Duration>, Duration), ContractError> {
    let min = min
        .map(|min| {
            let valid = match (min, max) {
                (Duration::Time(min), Duration::Time(max)) => min <= max,
                (Duration::Height(min), Duration::Height(max)) => min <= max,
                _ => return Err(ContractError::DurationUnitsConflict {}),
            };
            if valid {
                Ok(min)
            } else {
                Err(ContractError::InvalidMinVotingPeriod {})
            }
        })
        .transpose()?;

    Ok((min, max))
}

pub fn get_sender_origin(
    deps: Deps,
    dao: Addr,
    relayed_from: Option<String>,
    sender: Addr,
) -> Result<Addr, ContractError> {
    match relayed_from {
        Some(addr) => {
            let dao_tunnel: cw_core::query::GetItemResponse = deps.querier.query_wasm_smart(
                dao.to_string(),
                &cw_core::msg::QueryMsg::GetItem {
                    key: "dao-tunnel".to_string(),
                },
            )?;
            if dao_tunnel.item.is_none() || dao_tunnel.item.unwrap() != sender.to_string() {
                Err(ContractError::Unauthorized {})
            } else {
                Ok(Addr::unchecked(addr))
            }
        }
        None => Ok(sender),
    }
}

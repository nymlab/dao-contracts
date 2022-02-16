use crate::{proposal::MultipleChoiceProposal, voting_strategy::VotingStrategy, ContractError};
use cosmwasm_std::{Addr, CosmosMsg, Empty, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;
use indexable_hooks::Hooks;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voting::{deposit::CheckedDepositInfo, voting::MultipleChoiceVote};

const MAX_NUM_CHOICES: u32 = 10;
const NONE_OPTION_DESCRIPTION: &str = "None of the Above";

/// The governance module's configuration.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// The threshold a proposal must reach to complete.
    pub voting_strategy: VotingStrategy,
    /// The default maximum amount of time a proposal may be voted on
    /// before expiring.
    pub max_voting_period: Duration,
    /// If set to true only members may execute passed
    /// proposals. Otherwise, any address may execute a passed
    /// proposal.
    pub only_members_execute: bool,
    /// The address of the DAO that this governance module is
    /// associated with.
    pub dao: Addr,
    /// Information about the depost required to create a
    /// proposal. None if no deposit is required, Some otherwise.
    pub deposit_info: Option<CheckedDepositInfo>,
}

/// Information about a vote that was cast.
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct VoteInfo {
    /// The address that voted.
    pub voter: Addr,
    /// Position on the vote.
    pub vote: MultipleChoiceVote,
    /// The voting power behind the vote.
    pub power: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum MultipleChoiceOptionType {
    /// Choice that represents selecting none of the options; still counts toward quorum
    /// and allows proposals with all bad options to be voted against.
    None,
    Standard,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MultipleChoiceOptions {
    pub options: Vec<MultipleChoiceOption>,
}

pub struct CheckedMultipleChoiceOptions {
    pub options: Vec<CheckedMultipleChoiceOption>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MultipleChoiceOption {
    pub description: String,
    pub msgs: Option<Vec<CosmosMsg<Empty>>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CheckedMultipleChoiceOption {
    // This is the index of the option in both the vote_weights and proposal.choices vectors.
    // Workaround due to not being able to use HashMaps in Cosmwasm.
    pub index: u32,
    pub option_type: MultipleChoiceOptionType,
    pub description: String,
    pub msgs: Option<Vec<CosmosMsg<Empty>>>,
    pub vote_count: Uint128,
}

impl MultipleChoiceOptions {
    pub fn into_checked(&self) -> Result<CheckedMultipleChoiceOptions, ContractError> {
        if self.options.len() < 3 || self.options.len() > MAX_NUM_CHOICES as usize {
            return Err(ContractError::WrongNumberOfChoices {});
        }

        let mut checked_options: Vec<CheckedMultipleChoiceOption> =
            Vec::with_capacity(self.options.len() + 1);

        // Iterate through choices and save the index and option type for each
        self.options.iter().enumerate().for_each(|(idx, choice)| {
            let checked_option = CheckedMultipleChoiceOption {
                index: idx as u32,
                option_type: MultipleChoiceOptionType::Standard,
                description: choice.description.clone(),
                msgs: choice.msgs.clone(),
                vote_count: Uint128::zero(),
            };
            checked_options.push(checked_option);
        });

        // Add a "None of the above" option, required for every multiple choice proposal.
        let none_option = CheckedMultipleChoiceOption {
            index: (checked_options.len() + 1) as u32,
            option_type: MultipleChoiceOptionType::None,
            description: NONE_OPTION_DESCRIPTION.to_string(),
            msgs: None,
            vote_count: Uint128::zero(),
        };

        checked_options.push(none_option);

        let options = CheckedMultipleChoiceOptions {
            options: checked_options,
        };
        Ok(options)
    }
}

// we cast a ballot with our chosen vote and a given weight
// stored under the key that voted
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Ballot {
    /// The amount of voting power behind the vote.
    pub power: Uint128,
    /// The position.
    pub vote: MultipleChoiceVote,
}

pub fn next_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = PROPOSAL_COUNT.may_load(store)?.unwrap_or_default() + 1;
    PROPOSAL_COUNT.save(store, &id)?;
    Ok(id)
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const PROPOSAL_COUNT: Item<u64> = Item::new("proposal_count");
pub const PROPOSALS: Map<u64, MultipleChoiceProposal> = Map::new("proposals");
pub const BALLOTS: Map<(u64, Addr), Ballot> = Map::new("ballots");
pub const PROPOSAL_HOOKS: Hooks = Hooks::new("proposal_hooks");
pub const VOTE_HOOKS: Hooks = Hooks::new("vote_hooks");

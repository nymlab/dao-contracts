use cosmwasm_std::{Addr, BlockInfo, StdError, StdResult, Uint128};
use cw_utils::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voting::{
    deposit::CheckedDepositInfo,
    proposal::Proposal,
    status::Status,
    voting::{does_vote_count_pass, MultipleChoiceVotes},
};

use crate::{
    query::ProposalResponse,
    state::{CheckedMultipleChoiceOption, MultipleChoiceOptionType},
    voting_strategy::VotingStrategy,
};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MultipleChoiceProposal {
    pub title: String,
    pub description: String,
    pub proposer: Addr,
    pub start_height: u64,
    pub expiration: Expiration,
    pub choices: Vec<CheckedMultipleChoiceOption>,
    pub status: Status,
    pub voting_strategy: VotingStrategy,
    /// The total power when the proposal started (used to calculate percentages)
    pub total_power: Uint128,
    pub votes: MultipleChoiceVotes,
    /// Information about the deposit that was sent as part of this
    /// proposal. None if no deposit.
    pub deposit_info: Option<CheckedDepositInfo>,
}

pub enum VoteResult {
    SingleWinner(CheckedMultipleChoiceOption),
    Tie,
}

impl Proposal for MultipleChoiceProposal {
    fn proposer(&self) -> Addr {
        self.proposer.clone()
    }
    fn deposit_info(&self) -> Option<CheckedDepositInfo> {
        self.deposit_info.clone()
    }
    fn status(&self) -> Status {
        self.status
    }
}

impl MultipleChoiceProposal {
    /// Consumes the proposal and returns a version which may be used
    /// in a query response. The difference being that proposal
    /// statuses are only updated on vote, execute, and close
    /// events. It is possible though that since a vote has occured
    /// the proposal expiring has changed its status. This method
    /// recomputes the status so that queries get accurate
    /// information.
    pub fn into_response(mut self, block: &BlockInfo, id: u64) -> StdResult<ProposalResponse> {
        self.update_status(block)?;
        Ok(ProposalResponse { id, proposal: self })
    }

    /// Gets the current status of the proposal.
    pub fn current_status(&self, block: &BlockInfo) -> StdResult<Status> {
        if self.status == Status::Open && self.is_passed(block)? {
            Ok(Status::Passed)
        } else if self.status == Status::Open
            && (self.expiration.is_expired(block) || self.is_rejected(block)?)
        {
            Ok(Status::Rejected)
        } else {
            Ok(self.status)
        }
    }

    /// Sets a proposals status to its current status.
    pub fn update_status(&mut self, block: &BlockInfo) -> StdResult<()> {
        self.status = self.current_status(block)?;
        Ok(())
    }

    /// Returns true iff this proposal is sure to pass (even before
    /// expiration if no future sequence of possible votes can cause
    /// it to fail). Passing in the case of multiple choice proposals
    /// means that one of the options that is not "None of the above"
    /// has won the most votes, and there is no tie.
    pub fn is_passed(&self, block: &BlockInfo) -> StdResult<bool> {
        // Proposal can only pass if quorum has been met.
        if does_vote_count_pass(
            self.votes.total(),
            self.total_power,
            self.voting_strategy.get_quorum(),
        ) {
            let vote_result = self.calculate_vote_result()?;
            match vote_result {
                // Proposal is not passed if there is a tie.
                VoteResult::Tie => return Ok(false),
                VoteResult::SingleWinner(winning_choice) => {
                    // Proposal is not passed if winning choice is None.
                    if winning_choice.option_type != MultipleChoiceOptionType::None {
                        // If proposal is expired, quorum has been reached, and winning choice is neither tied nor None, then proposal is passed.
                        if self.expiration.is_expired(block) {
                            return Ok(true);
                        } else {
                            // If the proposal is not expired but the leading choice cannot
                            // possibly be outwon by any other choices, the proposal has passed.
                            return self.is_choice_unbeatable(&winning_choice);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    pub fn is_rejected(&self, block: &BlockInfo) -> StdResult<bool> {
        let vote_result = self.calculate_vote_result()?;
        match vote_result {
            // Proposal is rejected if there is a tie.
            VoteResult::Tie => Ok(self.expiration.is_expired(block)),
            VoteResult::SingleWinner(winning_choice) => {
                match (
                    does_vote_count_pass(
                        self.votes.total(),
                        self.total_power,
                        self.voting_strategy.get_quorum(),
                    ),
                    self.expiration.is_expired(block),
                ) {
                    // Quorum is met and proposal is expired.
                    (true, true) => {
                        // Proposal is rejected if "None" is the winning option.
                        if winning_choice.option_type == MultipleChoiceOptionType::None {
                            return Ok(true);
                        }
                        Ok(false)
                    }
                    // Proposal is not expired, quorum is either is met or unmet.
                    (true, false) | (false, false) => {
                        // If the proposal is not expired and the leading choice is None and it cannot
                        // possibly be outwon by any other choices, the proposal is rejected.
                        if winning_choice.option_type == MultipleChoiceOptionType::None {
                            return self.is_choice_unbeatable(&winning_choice);
                        }
                        Ok(false)
                    }
                    // Quorum is not met and proposal is expired.
                    (false, true) => Ok(true),
                }
            }
        }
    }

    /// Find the option with the highest vote weight, and note if there is a tie.
    pub fn calculate_vote_result(&self) -> StdResult<VoteResult> {
        match self.voting_strategy {
            VotingStrategy::SingleChoice { quorum: _ } => {
                // We expect to have at least 3 vote weights
                if let Some(max_weight) = self.votes.vote_weights.iter().max_by(|&a, &b| a.cmp(b)) {
                    let top_choices: Vec<(usize, &Uint128)> = self
                        .votes
                        .vote_weights
                        .iter()
                        .enumerate()
                        .filter(|x| x.1 == max_weight)
                        .collect();

                    // If more than one choice has the highest number of votes, we have a tie.
                    if top_choices.len() > 1 {
                        return Ok(VoteResult::Tie);
                    }

                    match top_choices.first() {
                        Some(winning_choice) => {
                            return Ok(VoteResult::SingleWinner(
                                self.choices[winning_choice.0].clone(),
                            ));
                        }
                        None => {
                            return Err(StdError::generic_err("no votes found"));
                        }
                    }
                }
                Err(StdError::not_found("max vote weight"))
            }

            VotingStrategy::RankedChoice { quorum: _ } => todo!(),
        }
    }

    /// Ensure that with the remaining vote power, the choice with the second highest votes
    /// cannot overtake the first choice.
    fn is_choice_unbeatable(
        &self,
        winning_choice: &CheckedMultipleChoiceOption,
    ) -> StdResult<bool> {
        let winning_choice_power = self.votes.vote_weights[winning_choice.index as usize];
        if let Some(second_choice_power) = self
            .votes
            .vote_weights
            .iter()
            .filter(|&x| x < &winning_choice_power)
            .max_by(|&a, &b| a.cmp(b))
        {
            // Check if the remaining vote power can be used to overtake the current winning choice.
            let remaining_vote_power = self.total_power - self.votes.total();
            if winning_choice_power > *second_choice_power + remaining_vote_power {
                return Ok(true);
            }
        } else {
            return Err(StdError::not_found("second highest vote weight"));
        }
        Ok(false)
    }
}

//use cosmwasm_schema::write_api;
use cosmwasm_schema::write_api;
use dao_proposal_single::msg::{
    ProposalSingleExecuteMsg, ProposalSingleInstantiateMsg, ProposalSingleMigrateMsg,
    ProposalSingleQueryMsg,
};

fn main() {
    write_api! {
        instantiate: ProposalSingleInstantiateMsg,
        query: ProposalSingleQueryMsg,
        execute: ProposalSingleExecuteMsg,
        migrate: ProposalSingleMigrateMsg,
    }
}

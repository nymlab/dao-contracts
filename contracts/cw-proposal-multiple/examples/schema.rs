use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, export_schema_with_title, remove_schemas, schema_for};
use cw_proposal_multiple::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    proposal::MultipleChoiceProposal,
    query::{ProposalListResponse, ProposalResponse, VoteResponse},
    state::{Config, VoteInfo},
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema_with_title(&schema_for!(ExecuteMsg), &out_dir, "ExecuteMsg");
    export_schema_with_title(&schema_for!(QueryMsg), &out_dir, "QueryMsg");

    export_schema_with_title(&schema_for!(Config), &out_dir, "Config");
    export_schema_with_title(&schema_for!(MultipleChoiceProposal), &out_dir, "Proposal");
    export_schema_with_title(&schema_for!(ProposalResponse), &out_dir, "ProposalResponse");
    export_schema_with_title(
        &schema_for!(ProposalListResponse),
        &out_dir,
        "ProposalListResponse",
    );
    export_schema_with_title(&schema_for!(VoteInfo), &out_dir, "VoteInfo");
    export_schema_with_title(&schema_for!(VoteResponse), &out_dir, "VoteResponse");
}

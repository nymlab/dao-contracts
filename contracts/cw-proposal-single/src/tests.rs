use std::u128;

use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, Empty, Uint128, WasmMsg};
use cw20::Cw20Coin;
use cw20_staked_balance_voting::msg::ActiveThreshold;
use cw_multi_test::{next_block, App, Contract, ContractWrapper, Executor};

use cw_core::msg::ModuleInstantiateInfo;
use cw_utils::Duration;

use indexable_hooks::HooksResponse;

use testing::{ShouldExecute, TestVote};
use voting::{PercentageThreshold, Status, Threshold, Vote, Votes};

use crate::{
    msg::{DepositInfo, DepositToken, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    proposal::Proposal,
    query::{ProposalListResponse, ProposalResponse, VoteInfo, VoteResponse},
    state::{CheckedDepositInfo, Config},
    ContractError,
};

use vectis_govec::msg::{
    ExecuteMsg as GovecExecMsg, InstantiateMsg as GovecInstMsg, QueryMsg as GovecQueryMsg,
    UpdateAddrReq,
};

const CREATOR_ADDR: &str = "creator";

fn govec_cw20_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        vectis_govec::contract::execute,
        vectis_govec::contract::instantiate,
        vectis_govec::contract::query,
    );
    Box::new(contract)
}

fn cw20_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

fn cw20_stake_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake::contract::execute,
        cw20_stake::contract::instantiate,
        cw20_stake::contract::query,
    );
    Box::new(contract)
}

fn single_proposal_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::contract::reply)
    .with_migrate(crate::contract::migrate);
    Box::new(contract)
}

fn cw20_balances_voting() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_balance_voting::contract::execute,
        cw20_balance_voting::contract::instantiate,
        cw20_balance_voting::contract::query,
    )
    .with_reply(cw20_balance_voting::contract::reply);
    Box::new(contract)
}

fn cw20_staked_balances_voting() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_staked_balance_voting::contract::execute,
        cw20_staked_balance_voting::contract::instantiate,
        cw20_staked_balance_voting::contract::query,
    )
    .with_reply(cw20_staked_balance_voting::contract::reply);
    Box::new(contract)
}

fn cw_gov_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_core::contract::execute,
        cw_core::contract::instantiate,
        cw_core::contract::query,
    )
    .with_reply(cw_core::contract::reply);
    Box::new(contract)
}

fn staked_balances_voting() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_staked_balance_voting::contract::execute,
        cw20_staked_balance_voting::contract::instantiate,
        cw20_staked_balance_voting::contract::query,
    )
    .with_reply(cw20_staked_balance_voting::contract::reply);
    Box::new(contract)
}

fn cw20_stake() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_stake::contract::execute,
        cw20_stake::contract::instantiate,
        cw20_stake::contract::query,
    );
    Box::new(contract)
}

fn cw4_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw4_group::contract::execute,
        cw4_group::contract::instantiate,
        cw4_group::contract::query,
    );
    Box::new(contract)
}

fn cw4_voting_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw4_voting::contract::execute,
        cw4_voting::contract::instantiate,
        cw4_voting::contract::query,
    )
    .with_reply(cw4_voting::contract::reply);
    Box::new(contract)
}

fn instantiate_with_staked_balances_governance(
    app: &mut App,
    governance_code_id: u64,
    governance_instantiate: InstantiateMsg,
    initial_balances: Option<Vec<Cw20Coin>>,
) -> Addr {
    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    // Collapse balances so that we can test double votes.
    let initial_balances: Vec<Cw20Coin> = {
        let mut already_seen = vec![];
        initial_balances
            .into_iter()
            .filter(|Cw20Coin { address, amount: _ }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .collect()
    };

    let cw20_id = app.store_code(govec_cw20_contract());
    let cw20_stake_id = app.store_code(cw20_stake());
    let staked_balances_voting_id = app.store_code(staked_balances_voting());
    let core_contract_id = app.store_code(cw_gov_contract());

    let govec_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &GovecInstMsg {
                name: "Govec".to_string(),
                symbol: "GOV".to_string(),
                initial_balances: initial_balances.clone(),
                staking_addr: None,
                mint_cap: None,
                factory: None,
                dao_tunnel: None,
                marketing: None,
            },
            &[],
            "Govec",
            None,
        )
        .unwrap();

    let instantiate_core = cw_core::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: false,
        voting_module_instantiate_info: ModuleInstantiateInfo {
            code_id: staked_balances_voting_id,
            msg: to_binary(&cw20_staked_balance_voting::msg::InstantiateMsg {
                active_threshold: None,
                token_info: cw20_staked_balance_voting::msg::TokenInfo::Existing {
                    address: govec_addr.to_string(),
                    staking_contract: cw20_staked_balance_voting::msg::StakingInfo::New {
                        staking_code_id: cw20_stake_id,
                        unstaking_duration: None,
                    },
                },
            })
            .unwrap(),
            admin: cw_core::msg::Admin::None {},
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![ModuleInstantiateInfo {
            code_id: governance_code_id,
            label: "DAO DAO governance module.".to_string(),
            admin: cw_core::msg::Admin::CoreContract {},
            msg: to_binary(&governance_instantiate).unwrap(),
        }],
        initial_items: None,
    };

    let core_addr = app
        .instantiate_contract(
            core_contract_id,
            Addr::unchecked(CREATOR_ADDR),
            &instantiate_core,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr.clone(), &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let voting_module = gov_state.voting_module;

    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    app.execute(
        Addr::unchecked(CREATOR_ADDR),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: govec_addr.to_string(),
            msg: to_binary(&GovecExecMsg::UpdateConfigAddr {
                new_addr: UpdateAddrReq::Staking(staking_contract.to_string()),
            })
            .unwrap(),
            funds: vec![],
        }),
    )
    .unwrap();

    for proposal_mod in gov_state.proposal_modules {
        app.execute(
            Addr::unchecked(CREATOR_ADDR),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: govec_addr.to_string(),
                msg: to_binary(&GovecExecMsg::UpdateConfigAddr {
                    new_addr: UpdateAddrReq::Proposal(proposal_mod.to_string()),
                })
                .unwrap(),
                funds: vec![],
            }),
        )
        .unwrap();
    }

    // Stake all the initial balances.
    for Cw20Coin { address, amount } in initial_balances {
        app.execute_contract(
            Addr::unchecked(&address),
            token_contract.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: staking_contract.to_string(),
                amount,
                msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
            },
            &[],
        )
        .unwrap();
    }

    // Update the block so that those staked balances appear.
    app.update_block(|block| block.height += 1);

    core_addr
}

fn instantiate_with_staking_active_threshold(
    app: &mut App,
    code_id: u64,
    msg: InstantiateMsg,
    initial_balances: Option<Vec<Cw20Coin>>,
    active_threshold: Option<ActiveThreshold>,
) -> Addr {
    let cw20_id = app.store_code(govec_cw20_contract());
    let cw20_staking_id = app.store_code(cw20_stake_contract());
    let governance_id = app.store_code(cw_gov_contract());
    let votemod_id = app.store_code(cw20_staked_balances_voting());

    let initial_balances = initial_balances.unwrap_or_else(|| {
        vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100_000_000),
        }]
    });

    let govec_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &GovecInstMsg {
                name: "Govec".to_string(),
                symbol: "GOV".to_string(),
                initial_balances: initial_balances.clone(),
                staking_addr: None,
                mint_cap: None,
                factory: None,
                dao_tunnel: None,
                marketing: None,
            },
            &[],
            "Govec",
            None,
        )
        .unwrap();

    let governance_instantiate = cw_core::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: cw_core::msg::ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_binary(&cw20_staked_balance_voting::msg::InstantiateMsg {
                token_info: cw20_staked_balance_voting::msg::TokenInfo::Existing {
                    address: govec_addr.to_string(),
                    staking_contract: cw20_staked_balance_voting::msg::StakingInfo::New {
                        staking_code_id: cw20_staking_id,
                        unstaking_duration: None,
                    },
                },
                active_threshold,
            })
            .unwrap(),
            admin: cw_core::msg::Admin::CoreContract {},
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![cw_core::msg::ModuleInstantiateInfo {
            code_id,
            msg: to_binary(&msg).unwrap(),
            admin: cw_core::msg::Admin::CoreContract {},
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let governance_addr = app
        .instantiate_contract(
            governance_id,
            Addr::unchecked(CREATOR_ADDR),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    // Add proposal module to Govec to allow for `ProposalTransfer`
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    app.execute(
        Addr::unchecked(CREATOR_ADDR),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: govec_addr.to_string(),
            msg: to_binary(&GovecExecMsg::UpdateConfigAddr {
                new_addr: UpdateAddrReq::Proposal(governance_modules[0].to_string()),
            })
            .unwrap(),
            funds: vec![],
        }),
    )
    .unwrap();

    // Add staking contract to Govec to allow for Send
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::VotingModule {},
        )
        .unwrap();

    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();

    app.execute(
        Addr::unchecked(CREATOR_ADDR),
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: govec_addr.to_string(),
            msg: to_binary(&GovecExecMsg::UpdateConfigAddr {
                new_addr: UpdateAddrReq::Staking(staking_contract.to_string()),
            })
            .unwrap(),
            funds: vec![],
        }),
    )
    .unwrap();

    governance_addr
}

fn instantiate_with_cw4_groups_governance(
    app: &mut App,
    governance_code_id: u64,
    governance_instantiate: InstantiateMsg,
    initial_weights: Option<Vec<Cw20Coin>>,
) -> Addr {
    let cw4_id = app.store_code(cw4_contract());
    let core_id = app.store_code(cw_gov_contract());
    let votemod_id = app.store_code(cw4_voting_contract());

    let initial_weights = initial_weights.unwrap_or_default();

    // Remove duplicates so that we can test duplicate voting.
    let initial_weights: Vec<cw4::Member> = {
        let mut already_seen = vec![];
        initial_weights
            .into_iter()
            .filter(|Cw20Coin { address, .. }| {
                if already_seen.contains(address) {
                    false
                } else {
                    already_seen.push(address.clone());
                    true
                }
            })
            .map(|Cw20Coin { address, amount }| cw4::Member {
                addr: address,
                weight: amount.u128() as u64,
            })
            .collect()
    };

    let governance_instantiate = cw_core::msg::InstantiateMsg {
        admin: None,
        name: "DAO DAO".to_string(),
        description: "A DAO that builds DAOs".to_string(),
        image_url: None,
        automatically_add_cw20s: true,
        automatically_add_cw721s: true,
        voting_module_instantiate_info: cw_core::msg::ModuleInstantiateInfo {
            code_id: votemod_id,
            msg: to_binary(&cw4_voting::msg::InstantiateMsg {
                cw4_group_code_id: cw4_id,
                initial_members: initial_weights,
            })
            .unwrap(),
            admin: cw_core::msg::Admin::CoreContract {},
            label: "DAO DAO voting module".to_string(),
        },
        proposal_modules_instantiate_info: vec![cw_core::msg::ModuleInstantiateInfo {
            code_id: governance_code_id,
            msg: to_binary(&governance_instantiate).unwrap(),
            admin: cw_core::msg::Admin::CoreContract {},
            label: "DAO DAO governance module".to_string(),
        }],
        initial_items: None,
    };

    let addr = app
        .instantiate_contract(
            core_id,
            Addr::unchecked(CREATOR_ADDR),
            &governance_instantiate,
            &[],
            "DAO DAO",
            None,
        )
        .unwrap();

    // Update the block so that weights appear.
    app.update_block(|block| block.height += 1);

    addr
}

fn do_votes_staked_balances(
    votes: Vec<TestVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
) {
    do_test_votes(
        votes,
        threshold,
        expected_status,
        total_supply,
        None,
        instantiate_with_staked_balances_governance,
    );
}

fn do_votes_cw4_weights(
    votes: Vec<TestVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
) {
    do_test_votes(
        votes,
        threshold,
        expected_status,
        total_supply,
        None,
        instantiate_with_cw4_groups_governance,
    );
}

fn do_test_votes<F>(
    votes: Vec<TestVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
    deposit_info: Option<DepositInfo>,
    setup_governance: F,
) -> (App, Addr)
where
    F: Fn(&mut App, u64, InstantiateMsg, Option<Vec<Cw20Coin>>) -> Addr,
{
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let mut initial_balances = votes
        .iter()
        .map(|TestVote { voter, weight, .. }| Cw20Coin {
            address: voter.to_string(),
            amount: *weight,
        })
        .collect::<Vec<Cw20Coin>>();
    let initial_balances_supply = votes.iter().fold(Uint128::zero(), |p, n| p + n.weight);
    let to_fill = total_supply.map(|total_supply| total_supply - initial_balances_supply);
    if let Some(fill) = to_fill {
        initial_balances.push(Cw20Coin {
            address: "filler".to_string(),
            amount: fill,
        })
    }

    let proposer = match votes.first() {
        Some(vote) => vote.voter.clone(),
        None => panic!("do_test_votes must have at least one vote."),
    };

    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info,
    };

    let governance_addr =
        setup_governance(&mut app, govmod_id, instantiate, Some(initial_balances));

    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();

    if let Some(CheckedDepositInfo { deposit, .. }) = config.deposit_info {
        // We send deposit amount to the proposer to propose
        let vote: Addr = app
            .wrap()
            .query_wasm_smart(
                governance_addr.clone(),
                &cw_core::msg::QueryMsg::VotingModule {},
            )
            .unwrap();
        let govec_addr: Addr = app
            .wrap()
            .query_wasm_smart(vote, &cw_core_interface::voting::Query::TokenContract {})
            .unwrap();

        let staking_contract: Addr = app
            .wrap()
            .query_wasm_smart(govec_addr.clone(), &GovecQueryMsg::Staking {})
            .unwrap();

        if deposit != Uint128::zero() {
            app.execute_contract(
                Addr::unchecked(&proposer),
                staking_contract,
                &cw20_stake::msg::ExecuteMsg::Unstake {
                    amount: deposit,
                    relayed_from: None,
                },
                &[],
            )
            .unwrap();
        }

        // Update the block so that those staked balances appear.
        app.update_block(|block| block.height += 1);
    }

    app.execute_contract(
        Addr::unchecked(&proposer),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Cast votes.
    for vote in votes {
        let TestVote {
            voter,
            position,
            weight,
            should_execute,
        } = vote;
        // Vote on the proposal.
        let res = app.execute_contract(
            Addr::unchecked(voter.clone()),
            govmod_single.clone(),
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: position,
                relayed_from: None,
            },
            &[],
        );
        match should_execute {
            ShouldExecute::Yes => {
                assert!(res.is_ok());
                // Check that the vote was recorded correctly.
                let vote: VoteResponse = app
                    .wrap()
                    .query_wasm_smart(
                        govmod_single.clone(),
                        &QueryMsg::Vote {
                            proposal_id: 1,
                            voter: voter.clone(),
                        },
                    )
                    .unwrap();
                let expected = VoteResponse {
                    vote: Some(VoteInfo {
                        voter: Addr::unchecked(&voter),
                        vote: position,
                        power: match config.deposit_info {
                            Some(CheckedDepositInfo { deposit, .. }) => {
                                if proposer == voter {
                                    weight - deposit
                                } else {
                                    weight
                                }
                            }
                            None => weight,
                        },
                    }),
                };
                assert_eq!(vote, expected)
            }
            ShouldExecute::No => assert!(res.is_err()),
            ShouldExecute::Meh => (),
        }
    }

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod_single, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, expected_status);

    (app, governance_addr)
}

// Creates a proposal and then executes a series of votes on those
// proposals. Asserts both that those votes execute as expected and
// that the final status of the proposal is what is expected. Returns
// the address of the governance contract that it has created so that
// callers may do additional inspection of the contract's state.
fn do_test_votes_cw20_balances(
    votes: Vec<TestVote>,
    threshold: Threshold,
    expected_status: Status,
    total_supply: Option<Uint128>,
    deposit_info: Option<DepositInfo>,
) -> (App, Addr) {
    do_test_votes(
        votes,
        threshold,
        expected_status,
        total_supply,
        deposit_info,
        instantiate_with_staked_balances_governance,
    )
}

#[test]
fn test_propose() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold: threshold.clone(),
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr =
        instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    // Check that the governance module has been configured correctly.
    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let expected = Config {
        threshold: threshold.clone(),
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        dao: governance_addr,
        deposit_info: None,
    };
    assert_eq!(config, expected);

    // Create a new proposal.
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let created: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod_single, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    let current_block = app.block_info();
    let expected = Proposal {
        title: "A simple text proposal".to_string(),
        description: "This is a simple text proposal".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: max_voting_period.after(&current_block),
        min_voting_period: None,
        threshold,
        allow_revoting: false,
        total_power: Uint128::new(100_000_000),
        msgs: vec![],
        status: Status::Open,
        votes: Votes::zero(),
        deposit_info: None,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);
}

#[test]
fn test_propose_supports_stargate_message() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold: threshold.clone(),
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr =
        instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    // Create a new proposal.
    app.execute_contract(
        Addr::unchecked(CREATOR_ADDR),
        govmod_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            msgs: vec![CosmosMsg::Stargate {
                type_url: "foo_type".to_string(),
                value: to_binary("foo_bin").unwrap(),
            }],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let created: ProposalResponse = app
        .wrap()
        .query_wasm_smart(govmod_single, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    let current_block = app.block_info();
    let expected = Proposal {
        title: "A simple text proposal".to_string(),
        description: "This is a simple text proposal".to_string(),
        proposer: Addr::unchecked(CREATOR_ADDR),
        start_height: current_block.height,
        expiration: max_voting_period.after(&current_block),
        min_voting_period: None,
        threshold,
        allow_revoting: false,
        total_power: Uint128::new(100_000_000),
        msgs: vec![CosmosMsg::Stargate {
            type_url: "foo_type".to_string(),
            value: to_binary("foo_bin").unwrap(),
        }],
        status: Status::Open,
        votes: Votes::zero(),
        deposit_info: None,
    };

    assert_eq!(created.proposal, expected);
    assert_eq!(created.id, 1u64);
}

#[test]
fn test_vote_simple() {
    testing::test_simple_votes(do_votes_cw4_weights);
    testing::test_simple_votes(do_votes_staked_balances)
}

#[test]
fn test_simple_vote_no_overflow() {
    testing::test_simple_vote_no_overflow(do_votes_staked_balances)
}

#[test]
fn test_vote_no_overflow() {
    testing::test_vote_no_overflow(do_votes_staked_balances)
}

#[test]
fn test_simple_early_rejection() {
    testing::test_simple_early_rejection(do_votes_cw4_weights);
    testing::test_simple_early_rejection(do_votes_staked_balances)
}

#[test]
fn test_vote_abstain_only() {
    testing::test_vote_abstain_only(do_votes_cw4_weights);
    testing::test_vote_abstain_only(do_votes_staked_balances)
}

#[test]
fn test_tricky_rounding() {
    testing::test_tricky_rounding(do_votes_cw4_weights);
    testing::test_tricky_rounding(do_votes_staked_balances)
}

#[test]
fn test_no_double_votes() {
    testing::test_no_double_votes(do_votes_cw4_weights);
    testing::test_no_double_votes(do_votes_staked_balances);
}

#[test]
fn test_votes_favor_yes() {
    testing::test_votes_favor_yes(do_votes_staked_balances);
}

#[test]
fn test_votes_low_threshold() {
    testing::test_votes_low_threshold(do_votes_cw4_weights);
    testing::test_votes_low_threshold(do_votes_staked_balances)
}

#[test]
fn test_majority_vs_half() {
    testing::test_majority_vs_half(do_votes_cw4_weights);
    testing::test_majority_vs_half(do_votes_staked_balances)
}

#[test]
fn test_pass_threshold_not_quorum() {
    testing::test_pass_threshold_not_quorum(do_votes_cw4_weights);
    testing::test_pass_threshold_not_quorum(do_votes_staked_balances)
}

#[test]
fn test_pass_threshold_exactly_quorum() {
    testing::test_pass_exactly_quorum(do_votes_cw4_weights);
    testing::test_pass_exactly_quorum(do_votes_staked_balances);
}

/// Generate some random voting selections and make sure they behave
/// as expected.
#[test]
fn fuzz_voting() {
    testing::fuzz_voting(do_votes_staked_balances);
}

/// Instantiate the contract and use the voting module's token
/// contract as the proposal deposit token.
#[test]
fn test_voting_module_token_proposal_deposit_instantiate() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
    };

    let governance_addr =
        instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;
    let voting_module = gov_state.voting_module;

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single, &QueryMsg::Config {})
        .unwrap();
    let expected_token: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    assert_eq!(
        config.deposit_info,
        Some(CheckedDepositInfo {
            token: expected_token,
            deposit: Uint128::new(1),
            refund_failed_proposals: true
        })
    )
}

/// Instantiate the contract and use a cw20 unrealated to the voting
/// module for the proposal deposit.
// This is not supported in Vectis
#[test]
#[ignore]
fn test_different_token_proposal_deposit() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    let cw20_id = app.store_code(cw20_contract());
    let cw20_addr = app
        .instantiate_contract(
            cw20_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_base::msg::InstantiateMsg {
                name: "OAD OAD".to_string(),
                symbol: "OAD".to_string(),
                decimals: 6,
                initial_balances: vec![],
                mint: None,
                marketing: None,
            },
            &[],
            "random-cw20",
            None,
        )
        .unwrap();

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: Some(DepositInfo {
            token: DepositToken::Token {
                address: cw20_addr.to_string(),
            },
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
    };

    instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);
}

/// Try to instantiate the governance module with a non-cw20 as its
/// proposal deposit token. This should error as the `TokenInfo {}`
/// query ought to fail.
#[test]
#[should_panic(expected = "Error parsing into type cw20_balance_voting::msg::QueryMsg")]
fn test_bad_token_proposal_deposit() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    let cw20_id = app.store_code(cw20_contract());
    let votemod_id = app.store_code(cw20_balances_voting());

    let votemod_addr = app
        .instantiate_contract(
            votemod_id,
            Addr::unchecked(CREATOR_ADDR),
            &cw20_balance_voting::msg::InstantiateMsg {
                token_info: cw20_balance_voting::msg::TokenInfo::New {
                    code_id: cw20_id,
                    label: "DAO DAO governance token".to_string(),
                    name: "DAO".to_string(),
                    symbol: "DAO".to_string(),
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: CREATOR_ADDR.to_string(),
                        amount: Uint128::new(1),
                    }],
                    marketing: None,
                },
            },
            &[],
            "random-vote-module",
            None,
        )
        .unwrap();

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: Some(DepositInfo {
            token: DepositToken::Token {
                address: votemod_addr.to_string(),
            },
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
    };

    instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);
}

// Vectis Govec does not need allowance for deposit
#[test]
#[ignore]
fn test_take_proposal_deposit() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: true,
        }),
    };

    let governance_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        instantiate,
        Some(vec![Cw20Coin {
            address: "ekez".to_string(),
            amount: Uint128::new(2),
        }]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo {
        token,
        deposit,
        refund_failed_proposals,
    } = govmod_config.deposit_info.unwrap();
    assert!(refund_failed_proposals);
    assert_eq!(deposit, Uint128::new(1));

    app.execute_contract(
        Addr::unchecked("ekez"),
        govmod_single,
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Check that our balance was deducted.
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();
    assert_eq!(balance.balance, Uint128::new(1))
}

#[test]
fn test_deposit_return_on_execute() {
    // Will create a proposal and execute it, one token will be
    // deposited to create said proposal, expectation is that the
    // token is then returned once the proposal is executed.
    let deposit_amount = Uint128::new(1);
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestVote {
            voter: "ekez".to_string(),
            position: Vote::Yes,
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(90)),
        },
        Status::Passed,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: deposit_amount,
            refund_failed_proposals: false,
        }),
    );
    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();

    // Proposal has not been executed so deposit has not been
    // refunded.
    assert_eq!(balance.balance, Uint128::zero());

    // Execute the proposal, this should cause the deposit to be
    // refunded.
    app.execute_contract(
        Addr::unchecked("ekez"),
        govmod_single,
        &ExecuteMsg::Execute {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();

    // Proposal has been executed so deposit has been refunded.
    assert_eq!(balance.balance, deposit_amount);
}

#[test]
fn test_close_open_proposal() {
    let deposit_amount = Uint128::new(1);
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestVote {
            voter: "ekez".to_string(),
            position: Vote::No,
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(90)),
        },
        Status::Open,
        Some(Uint128::new(100)),
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: deposit_amount,
            refund_failed_proposals: true,
        }),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    // Check there is initially no balance as gone to deposit
    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();
    assert_eq!(balance.balance, Uint128::zero());

    // Close the proposal, this should error as the proposal is still
    // open and not expired.
    app.execute_contract(
        Addr::unchecked("keze"),
        govmod_single.clone(),
        &ExecuteMsg::Close {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap_err();

    // Make the proposal expire.
    app.update_block(|block| block.height += 10);

    // Close the proposal, this should work as the proposal is now
    // open and expired.
    app.execute_contract(
        Addr::unchecked("keze"),
        govmod_single.clone(),
        &ExecuteMsg::Close {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Check that a refund was issued.
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();

    // Proposal has been closed so deposit has been refunded.
    assert_eq!(balance.balance, deposit_amount);
}

#[test]
fn test_zero_deposit() {
    do_test_votes_cw20_balances(
        vec![TestVote {
            voter: "ekez".to_string(),
            position: Vote::Yes,
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(90)),
        },
        Status::Passed,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(0),
            refund_failed_proposals: false,
        }),
    );
}

#[test]
fn test_deposit_return_on_close() {
    let deposit_amount = Uint128::new(1);
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestVote {
            voter: "ekez".to_string(),
            position: Vote::No,
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(90)),
        },
        Status::Rejected,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: deposit_amount,
            refund_failed_proposals: true,
        }),
    );
    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();
    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();

    // Proposal has not been closed so deposit has not been
    // refunded.
    assert_eq!(balance.balance, Uint128::zero());

    // Close the proposal, this should cause the deposit to be
    // refunded.
    app.execute_contract(
        Addr::unchecked("ekez"),
        govmod_single,
        &ExecuteMsg::Close {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();

    // Proposal has been closed so deposit has been refunded.
    assert_eq!(balance.balance, deposit_amount);
}

#[test]
fn test_execute_expired_proposal() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "innactive".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let proposal_single = proposal_modules.into_iter().next().unwrap();

    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_single.clone(),
        &ExecuteMsg::Propose {
            title: "This proposal will expire.".to_string(),
            description: "What will happen?".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Proposal has now reached quorum but should not be passed.
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Open);

    // Expire the proposal. It should now be passed as we had 100% yes
    // votes inside the quorum.
    app.update_block(|b| b.height += 10);

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Passed);

    // Try to close the proposal. This should fail as the proposal is
    // passed.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_single.clone(),
        &ExecuteMsg::Close {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap_err();

    // Check that we can execute the proposal despite the fact that it
    // is technically expired.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_single.clone(),
        &ExecuteMsg::Execute {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Can't execute more than once.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_single.clone(),
        &ExecuteMsg::Execute {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap_err();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Executed);
}

#[test]
fn test_update_config() {
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestVote {
            voter: "ekez".to_string(),
            position: Vote::No,
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(90)),
        },
        Status::Rejected,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: false,
        }),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(
        govmod_config.threshold,
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(90)),
        }
    );

    let dao = govmod_config.dao;

    // Attempt to update the config from a non-dao address. This
    // should fail as it is unauthorized.
    app.execute_contract(
        Addr::unchecked("ekez"),
        govmod_single.clone(),
        &ExecuteMsg::UpdateConfig {
            threshold: Threshold::AbsolutePercentage {
                percentage: PercentageThreshold::Majority {},
            },
            max_voting_period: cw_utils::Duration::Height(10),
            min_voting_period: None,
            only_members_execute: false,
            allow_revoting: false,
            dao: CREATOR_ADDR.to_string(),
            deposit_info: None,
        },
        &[],
    )
    .unwrap_err();

    // Update the config from the DAO address. This should succede.
    app.execute_contract(
        dao.clone(),
        govmod_single.clone(),
        &ExecuteMsg::UpdateConfig {
            threshold: Threshold::AbsolutePercentage {
                percentage: PercentageThreshold::Majority {},
            },
            max_voting_period: cw_utils::Duration::Height(10),
            min_voting_period: None,
            only_members_execute: false,
            allow_revoting: false,
            dao: CREATOR_ADDR.to_string(),
            deposit_info: None,
        },
        &[],
    )
    .unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();

    let expected = Config {
        threshold: Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Majority {},
        },
        max_voting_period: cw_utils::Duration::Height(10),
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        dao: Addr::unchecked(CREATOR_ADDR),
        deposit_info: None,
    };
    assert_eq!(govmod_config, expected);

    // As we have changed the DAO address updating the config using
    // the original one should now fail.
    app.execute_contract(
        dao,
        govmod_single,
        &ExecuteMsg::UpdateConfig {
            threshold: Threshold::AbsolutePercentage {
                percentage: PercentageThreshold::Majority {},
            },
            max_voting_period: cw_utils::Duration::Height(10),
            min_voting_period: None,
            only_members_execute: false,
            allow_revoting: false,
            dao: CREATOR_ADDR.to_string(),
            deposit_info: None,
        },
        &[],
    )
    .unwrap_err();
}

#[test]
fn test_no_return_if_no_refunds() {
    let (mut app, governance_addr) = do_test_votes_cw20_balances(
        vec![TestVote {
            voter: "ekez".to_string(),
            position: Vote::No,
            weight: Uint128::new(10),
            should_execute: ShouldExecute::Yes,
        }],
        Threshold::AbsolutePercentage {
            percentage: PercentageThreshold::Percent(Decimal::percent(90)),
        },
        Status::Rejected,
        None,
        Some(DepositInfo {
            token: DepositToken::VotingModuleToken {},
            deposit: Uint128::new(1),
            refund_failed_proposals: false,
        }),
    );
    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(governance_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let governance_modules = gov_state.proposal_modules;

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let CheckedDepositInfo { token, .. } = govmod_config.deposit_info.unwrap();

    // Close the proposal, this should cause the deposit to be
    // refunded.
    app.execute_contract(
        Addr::unchecked("ekez"),
        govmod_single,
        &ExecuteMsg::Close {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let balance: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            token,
            &cw20::Cw20QueryMsg::Balance {
                address: "ekez".to_string(),
            },
        )
        .unwrap();

    // Proposal has been closed but deposit has not been refunded.
    assert_eq!(balance.balance, Uint128::zero());
}

#[test]
fn test_query_list_proposals() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    let gov_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(0)),
            },
            max_voting_period: cw_utils::Duration::Height(100),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![Cw20Coin {
            address: CREATOR_ADDR.to_string(),
            amount: Uint128::new(100),
        }]),
    );

    let gov_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            gov_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(gov_modules.len(), 1);

    let govmod = gov_modules.into_iter().next().unwrap();

    for i in 1..10 {
        app.execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod.clone(),
            &ExecuteMsg::Propose {
                title: format!("Text proposal {}.", i),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap();
    }

    let proposals_forward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod.clone(),
            &QueryMsg::ListProposals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    let mut proposals_backward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod.clone(),
            &QueryMsg::ReverseProposals {
                start_before: None,
                limit: None,
            },
        )
        .unwrap();

    proposals_backward.proposals.reverse();

    assert_eq!(proposals_forward.proposals, proposals_backward.proposals);

    let expected = ProposalResponse {
        id: 1,
        proposal: Proposal {
            title: "Text proposal 1.".to_string(),
            description: "This is a simple text proposal".to_string(),
            proposer: Addr::unchecked(CREATOR_ADDR),
            start_height: app.block_info().height,
            expiration: cw_utils::Expiration::AtHeight(app.block_info().height + 100),
            min_voting_period: None,
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(0)),
            },
            allow_revoting: false,
            total_power: Uint128::new(100),
            msgs: vec![],
            status: Status::Open,
            votes: Votes::zero(),
            deposit_info: None,
        },
    };
    assert_eq!(proposals_forward.proposals[0], expected);

    // Get proposals (3, 5]
    let proposals_forward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod.clone(),
            &QueryMsg::ListProposals {
                start_after: Some(3),
                limit: Some(2),
            },
        )
        .unwrap();
    let mut proposals_backward: ProposalListResponse = app
        .wrap()
        .query_wasm_smart(
            govmod,
            &QueryMsg::ReverseProposals {
                start_before: Some(6),
                limit: Some(2),
            },
        )
        .unwrap();

    let expected = ProposalResponse {
        id: 4,
        proposal: Proposal {
            title: "Text proposal 4.".to_string(),
            description: "This is a simple text proposal".to_string(),
            proposer: Addr::unchecked(CREATOR_ADDR),
            start_height: app.block_info().height,
            expiration: cw_utils::Expiration::AtHeight(app.block_info().height + 100),
            min_voting_period: None,
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(0)),
            },
            allow_revoting: false,
            total_power: Uint128::new(100),
            msgs: vec![],
            status: Status::Open,
            votes: Votes::zero(),
            deposit_info: None,
        },
    };
    assert_eq!(proposals_forward.proposals[0], expected);
    assert_eq!(proposals_backward.proposals[1], expected);

    proposals_backward.proposals.reverse();
    assert_eq!(proposals_forward.proposals, proposals_backward.proposals);
}

#[test]
fn test_hooks() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr =
        instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;

    // Expect no hooks
    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::ProposalHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 0);

    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::VoteHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 0);

    let msg = ExecuteMsg::AddProposalHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &msg,
            &[],
        )
        .unwrap_err();

    // Expect success as sender is now DAO
    let _res = app
        .execute_contract(dao.clone(), govmod_single.clone(), &msg, &[])
        .unwrap();

    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::ProposalHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);

    // Expect error as hook is already set
    let _err = app
        .execute_contract(dao.clone(), govmod_single.clone(), &msg, &[])
        .unwrap_err();

    // Expect error as hook does not exist
    let _err = app
        .execute_contract(
            dao.clone(),
            govmod_single.clone(),
            &ExecuteMsg::RemoveProposalHook {
                address: "not_exist".to_string(),
            },
            &[],
        )
        .unwrap_err();

    let msg = ExecuteMsg::RemoveProposalHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &msg,
            &[],
        )
        .unwrap_err();

    // Expect success
    let _res = app
        .execute_contract(dao.clone(), govmod_single.clone(), &msg, &[])
        .unwrap();

    let msg = ExecuteMsg::AddVoteHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &msg,
            &[],
        )
        .unwrap_err();

    // Expect success as sender is now DAO
    let _res = app
        .execute_contract(dao.clone(), govmod_single.clone(), &msg, &[])
        .unwrap();

    let hooks: HooksResponse = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::VoteHooks {})
        .unwrap();
    assert_eq!(hooks.hooks.len(), 1);

    // Expect error as hook is already set
    let _err = app
        .execute_contract(dao.clone(), govmod_single.clone(), &msg, &[])
        .unwrap_err();

    // Expect error as hook does not exist
    let _err = app
        .execute_contract(
            dao.clone(),
            govmod_single.clone(),
            &ExecuteMsg::RemoveVoteHook {
                address: "not_exist".to_string(),
            },
            &[],
        )
        .unwrap_err();

    let msg = ExecuteMsg::RemoveVoteHook {
        address: "some_addr".to_string(),
    };

    // Expect error as sender is not DAO
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &msg,
            &[],
        )
        .unwrap_err();

    // Expect success
    let _res = app.execute_contract(dao, govmod_single, &msg, &[]).unwrap();
}

#[test]
fn test_active_threshold_absolute() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        govmod_id,
        instantiate,
        None,
        Some(ActiveThreshold::AbsoluteCount {
            count: Uint128::new(100),
        }),
    );
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Try and create a proposal, will fail as inactive
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap_err();

    // Stake enough tokens
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(100),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will now succeed as enough tokens are staked
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap();

    // Unstake some tokens to make it inactive again
    let msg = cw20_stake::msg::ExecuteMsg::Unstake {
        amount: Uint128::new(50),
        relayed_from: None,
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), staking_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will fail as no longer active
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single,
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap_err();
}

#[test]
fn test_active_threshold_percent() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    // 20% needed to be active, 20% of 100000000 is 20000000
    let governance_addr = instantiate_with_staking_active_threshold(
        &mut app,
        govmod_id,
        instantiate,
        None,
        Some(ActiveThreshold::Percentage {
            percent: Decimal::percent(20),
        }),
    );
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Try and create a proposal, will fail as inactive
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap_err();

    // Stake enough tokens
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(20000000),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will now succeed as enough tokens are staked
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single.clone(),
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap();

    // Unstake some tokens to make it inactive again
    let msg = cw20_stake::msg::ExecuteMsg::Unstake {
        amount: Uint128::new(1000),
        relayed_from: None,
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), staking_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will fail as no longer active
    let _err = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single,
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap_err();
}

#[test]
fn test_active_threshold_none() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr =
        instantiate_with_staking_active_threshold(&mut app, govmod_id, instantiate, None, None);
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let govmod_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();
    let dao = govmod_config.dao;
    let voting_module: Addr = app
        .wrap()
        .query_wasm_smart(dao, &cw_core::msg::QueryMsg::VotingModule {})
        .unwrap();
    let staking_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module.clone(),
            &cw20_staked_balance_voting::msg::QueryMsg::StakingContract {},
        )
        .unwrap();
    let token_contract: Addr = app
        .wrap()
        .query_wasm_smart(
            voting_module,
            &cw_core_interface::voting::Query::TokenContract {},
        )
        .unwrap();

    // Stake some tokens so we can propose
    let msg = cw20::Cw20ExecuteMsg::Send {
        contract: staking_contract.to_string(),
        amount: Uint128::new(2000),
        msg: to_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
    };
    app.execute_contract(Addr::unchecked(CREATOR_ADDR), token_contract, &msg, &[])
        .unwrap();
    app.update_block(next_block);

    // Try and create a proposal, will succeed as no threshold
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single,
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap();

    // Now try with balance voting to test when IsActive is not implemented
    // on the contract
    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr =
        instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr,
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    // Try and create a proposal, will succeed as IsActive is not implemented
    let _res = app
        .execute_contract(
            Addr::unchecked(CREATOR_ADDR),
            govmod_single,
            &crate::msg::ExecuteMsg::Propose {
                title: "A simple text proposal".to_string(),
                description: "This is a simple text proposal".to_string(),
                msgs: vec![],
                relayed_from: None,
            },
            &[],
        )
        .unwrap();
}

/// Simple test for revoting.
#[test]
fn test_revoting() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: true,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(90),
            },
            Cw20Coin {
                address: "slarbibfast".to_string(),
                amount: Uint128::new(10),
            },
        ]),
    );

    let core_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state.proposal_modules.into_iter().next().unwrap();

    // The supreme galatic floob rules over many DAOs with benevolance
    // and grace. The people of floob have become complacent in the
    // goodness of the floob.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "Supreme galactic floob.".to_string(),
            description: "Recognize the supreme galactic floob as our DAO leader.".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // The people initially jump at the chance to recognize the supreme
    // galactic floob!
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // The wise slarbibfast does not agree. After some digging they
    // discover that the floob has been bugging the hotel rooms of
    // political rivals.
    app.execute_contract(
        Addr::unchecked("slarbibfast"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Time passes.
    app.update_block(|b| b.height += 5);

    // Word spreads.
    app.update_block(|b| b.height += 4);

    // At the last moment the people realize their mistake.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Open);
    assert_eq!(
        proposal.proposal.votes,
        Votes {
            yes: Uint128::zero(),
            no: Uint128::new(100),
            abstain: Uint128::zero()
        }
    );

    // As the clock strikes midnight on the last day of the proposal,
    // revoting has saved the day!
    app.update_block(|b| b.height += 1);
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_module, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Rejected);
}

/// Tests that revoting is stored at a per-proposal level. Proposals
/// created while revoting is enabled should not have it disabled if a
/// config change turns if off.
#[test]
fn test_allow_revoting_config_changes() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: true,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(90),
            },
            Cw20Coin {
                address: "slarbibfast".to_string(),
                amount: Uint128::new(10),
            },
        ]),
    );

    let core_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr.clone(), &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state.proposal_modules.into_iter().next().unwrap();

    // Create a proposal. This proposal should allow revoting.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "Supreme galactic floob.".to_string(),
            description: "Recognize the supreme galactic floob as our DAO leader.".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Disable revoting.
    app.execute_contract(
        core_addr.clone(),
        proposal_module.clone(),
        &ExecuteMsg::UpdateConfig {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
            dao: core_addr.to_string(),
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    // The first created proposal should still allow revoting.
    assert!(proposal.proposal.allow_revoting);
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // New proposals should not allow revoting.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "Supreme galactic floob.".to_string(),
            description: "Recognize the supreme galactic floob as our DAO leader.".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("slarbibfast"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 2,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("slarbibfast"),
            proposal_module,
            &ExecuteMsg::Vote {
                proposal_id: 2,
                vote: Vote::Yes,
                relayed_from: None,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert!(matches!(err, ContractError::AlreadyVoted {}))
}

/// Tests that we error if a revote casts the same vote as the
/// previous vote.
#[test]
fn test_revoting_same_vote_twice() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: true,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(90),
            },
            Cw20Coin {
                address: "slarbibfast".to_string(),
                amount: Uint128::new(10),
            },
        ]),
    );

    let core_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state.proposal_modules.into_iter().next().unwrap();

    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "Supreme galactic floob.".to_string(),
            description: "Recognize the supreme galactic floob as our DAO leader.".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("ekez"),
            proposal_module.clone(),
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: Vote::Yes,
                relayed_from: None,
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    // Can't cast the same vote twice.
    assert!(matches!(err, ContractError::AlreadyCast {}));

    // Casting a different vote is fine. You can do that as many times
    // as you'd like.
    for _ in 1..5 {
        app.execute_contract(
            Addr::unchecked("ekez"),
            proposal_module.clone(),
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: Vote::No,
                relayed_from: None,
            },
            &[],
        )
        .unwrap();
        app.execute_contract(
            Addr::unchecked("ekez"),
            proposal_module.clone(),
            &ExecuteMsg::Vote {
                proposal_id: 1,
                vote: Vote::Yes,
                relayed_from: None,
            },
            &[],
        )
        .unwrap();
    }
}

/// Tests a simple three of five multisig configuration.
#[test]
fn test_three_of_five_multisig() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_cw4_groups_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::AbsoluteCount {
                threshold: Uint128::new(3),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "four".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "five".to_string(),
                amount: Uint128::new(1),
            },
        ]),
    );

    let core_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state.proposal_modules.into_iter().next().unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "Propose a thing.".to_string(),
            description: "Do the thing.".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("two"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Make sure it doesn't pass early.
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Open);

    app.execute_contract(
        Addr::unchecked("three"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Passed);

    app.execute_contract(
        Addr::unchecked("four"),
        proposal_module.clone(),
        &ExecuteMsg::Execute {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_module, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Executed);
}

/// Tests proposal rejection with three of five multisig style voting.
#[test]
fn test_three_of_five_multisig_reject() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_cw4_groups_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::AbsoluteCount {
                threshold: Uint128::new(3),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "four".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "five".to_string(),
                amount: Uint128::new(1),
            },
        ]),
    );

    let core_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state.proposal_modules.into_iter().next().unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "Propose a thing.".to_string(),
            description: "Do the thing.".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("two"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("three"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("four"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Still one vote outstanding but the module ought to have
    // rejected it already as that one vote can not make the proposal
    // pass.
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Rejected);

    app.execute_contract(
        Addr::unchecked("four"),
        proposal_module.clone(),
        &ExecuteMsg::Close {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_module, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Closed);
}

/// Tests that we fail to instantiate when using multisig style voting
/// power and `VotingModuleToken {}`.
#[test]
#[should_panic]
fn test_voting_module_token_with_multisig_style_voting() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    instantiate_with_cw4_groups_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::AbsoluteCount {
                threshold: Uint128::new(3),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: Some(DepositInfo {
                token: DepositToken::VotingModuleToken {},
                deposit: Uint128::new(1),
                refund_failed_proposals: true,
            }),
        },
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
        ]),
    );
}

/// Tests revoting with multisig style absolute count thresholds.
#[test]
fn test_three_of_five_multisig_revoting() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_cw4_groups_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::AbsoluteCount {
                threshold: Uint128::new(3),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: true,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "one".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "two".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "three".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "four".to_string(),
                amount: Uint128::new(1),
            },
            Cw20Coin {
                address: "five".to_string(),
                amount: Uint128::new(1),
            },
        ]),
    );

    let core_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_module = core_state.proposal_modules.into_iter().next().unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        proposal_module.clone(),
        &ExecuteMsg::Propose {
            title: "Propose a thing.".to_string(),
            description: "Do the thing.".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("one"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        Addr::unchecked("two"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("three"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("four"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Make sure it doesn't pass early.
    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Open);

    // Four changes their mind.
    app.execute_contract(
        Addr::unchecked("four"),
        proposal_module.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    app.update_block(|b| b.height += 10);

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_module.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Passed);

    app.execute_contract(
        Addr::unchecked("four"),
        proposal_module.clone(),
        &ExecuteMsg::Execute {
            proposal_id: 1,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_module, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.proposal.status, Status::Executed);
}

/// Tests that absolute count style thresholds work with token style
/// voting.
#[test]
fn test_absolute_count_threshold_non_multisig() {
    do_votes_staked_balances(
        vec![
            TestVote {
                voter: "one".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(10),
                should_execute: ShouldExecute::Yes,
            },
            TestVote {
                voter: "two".to_string(),
                position: Vote::No,
                weight: Uint128::new(200),
                should_execute: ShouldExecute::Yes,
            },
            TestVote {
                voter: "three".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
        ],
        Threshold::AbsoluteCount {
            threshold: Uint128::new(11),
        },
        Status::Passed,
        None,
    );
}

/// Tests that we do not overflow when faced with really high token /
/// vote supply.
#[test]
fn test_large_absolute_count_threshold() {
    do_votes_staked_balances(
        vec![
            // Instant rejection after this.
            TestVote {
                voter: "two".to_string(),
                position: Vote::No,
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
            TestVote {
                voter: "one".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(u128::MAX - 1),
                should_execute: ShouldExecute::No,
            },
        ],
        Threshold::AbsoluteCount {
            threshold: Uint128::new(u128::MAX),
        },
        Status::Rejected,
        None,
    );

    do_votes_staked_balances(
        vec![
            TestVote {
                voter: "one".to_string(),
                position: Vote::Yes,
                weight: Uint128::new(u128::MAX - 1),
                should_execute: ShouldExecute::Yes,
            },
            TestVote {
                voter: "two".to_string(),
                position: Vote::No,
                weight: Uint128::new(1),
                should_execute: ShouldExecute::Yes,
            },
        ],
        Threshold::AbsoluteCount {
            threshold: Uint128::new(u128::MAX),
        },
        Status::Rejected,
        None,
    );
}

#[test]
fn test_migrate() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());

    let threshold = Threshold::AbsolutePercentage {
        percentage: PercentageThreshold::Majority {},
    };
    let max_voting_period = cw_utils::Duration::Height(6);
    let instantiate = InstantiateMsg {
        threshold,
        max_voting_period,
        min_voting_period: None,
        only_members_execute: false,
        allow_revoting: false,
        deposit_info: None,
    };

    let governance_addr =
        instantiate_with_staked_balances_governance(&mut app, govmod_id, instantiate, None);
    let governance_modules: Vec<Addr> = app
        .wrap()
        .query_wasm_smart(
            governance_addr.clone(),
            &cw_core::msg::QueryMsg::ProposalModules {
                start_at: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(governance_modules.len(), 1);
    let govmod_single = governance_modules.into_iter().next().unwrap();

    let config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single.clone(), &QueryMsg::Config {})
        .unwrap();

    app.execute(
        governance_addr,
        CosmosMsg::Wasm(WasmMsg::Migrate {
            contract_addr: govmod_single.to_string(),
            new_code_id: govmod_id,
            msg: to_binary(&MigrateMsg {}).unwrap(),
        }),
    )
    .unwrap();

    let new_config: Config = app
        .wrap()
        .query_wasm_smart(govmod_single, &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config, new_config);
}

#[test]
fn test_proposal_count_initialized_to_zero() {
    let mut app = App::default();
    let proposal_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        proposal_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: None,
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "innactive".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let proposal_single = proposal_modules.into_iter().next().unwrap();

    let proposal_count: u64 = app
        .wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::ProposalCount {})
        .unwrap();
    assert_eq!(proposal_count, 0);
}

#[test]
fn test_no_early_pass_with_min_duration() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: Some(Duration::Height(2)),
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "wale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let proposal_single = proposal_modules.into_iter().next().unwrap();

    app.execute_contract(
        Addr::unchecked("wale"),
        proposal_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Wale votes yes which under normal curcumstances would cause the
    // proposal to pass. Because there is a min duration it does not.
    app.execute_contract(
        Addr::unchecked("wale"),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Open);

    // Let the min voting period pass.
    app.update_block(|b| b.height += 2);

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Passed);
}

#[test]
#[should_panic(
    expected = "min_voting_period and max_voting_period must have the same units (height or time)"
)]
fn test_min_duration_units_missmatch() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: Some(Duration::Time(2)),
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "wale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );
}

#[test]
#[should_panic(expected = "Min voting period must be less than or equal to max voting period")]
fn test_min_duration_larger_than_proposal_duration() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Height(10),
            min_voting_period: Some(Duration::Height(11)),
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "wale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );
}

#[test]
fn test_min_duration_same_as_proposal_duration() {
    let mut app = App::default();
    let govmod_id = app.store_code(single_proposal_contract());
    let core_addr = instantiate_with_staked_balances_governance(
        &mut app,
        govmod_id,
        InstantiateMsg {
            threshold: Threshold::ThresholdQuorum {
                threshold: PercentageThreshold::Majority {},
                quorum: PercentageThreshold::Percent(Decimal::percent(10)),
            },
            max_voting_period: Duration::Time(10),
            min_voting_period: Some(Duration::Time(10)),
            only_members_execute: true,
            allow_revoting: false,
            deposit_info: None,
        },
        Some(vec![
            Cw20Coin {
                address: "ekez".to_string(),
                amount: Uint128::new(10),
            },
            Cw20Coin {
                address: "wale".to_string(),
                amount: Uint128::new(90),
            },
        ]),
    );

    let gov_state: cw_core::query::DumpStateResponse = app
        .wrap()
        .query_wasm_smart(core_addr, &cw_core::msg::QueryMsg::DumpState {})
        .unwrap();
    let proposal_modules = gov_state.proposal_modules;

    assert_eq!(proposal_modules.len(), 1);
    let proposal_single = proposal_modules.into_iter().next().unwrap();

    app.execute_contract(
        Addr::unchecked("wale"),
        proposal_single.clone(),
        &ExecuteMsg::Propose {
            title: "A simple text proposal".to_string(),
            description: "This is a simple text proposal".to_string(),
            msgs: vec![],
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Wale votes yes which under normal curcumstances would cause the
    // proposal to pass. Because there is a min duration it does not.
    app.execute_contract(
        Addr::unchecked("wale"),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::Yes,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(
            proposal_single.clone(),
            &QueryMsg::Proposal { proposal_id: 1 },
        )
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Open);

    // ekez can vote no.
    app.execute_contract(
        Addr::unchecked("ekez"),
        proposal_single.clone(),
        &ExecuteMsg::Vote {
            proposal_id: 1,
            vote: Vote::No,
            relayed_from: None,
        },
        &[],
    )
    .unwrap();

    // Let the min voting period pass.
    app.update_block(|b| b.time = b.time.plus_seconds(10));

    let proposal: ProposalResponse = app
        .wrap()
        .query_wasm_smart(proposal_single, &QueryMsg::Proposal { proposal_id: 1 })
        .unwrap();

    assert_eq!(proposal.proposal.status, Status::Passed);
}

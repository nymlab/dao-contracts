use cosmwasm_schema::write_api;
use cw20_stake::msg::{
    Cw20StakeExecuteMsg, Cw20StakeInstantiateMsg, Cw20StakeMigrateMsg, Cw20StakeQueryMsg,
};

fn main() {
    write_api! {
        instantiate: Cw20StakeInstantiateMsg,
        query: Cw20StakeQueryMsg,
        execute: Cw20StakeExecuteMsg,
        migrate: Cw20StakeMigrateMsg,
    }
}

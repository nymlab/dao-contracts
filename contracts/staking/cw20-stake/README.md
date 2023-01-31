# CW20 Stake

_This is a fork of the original [cw20-stake] contract in the daodao v2 code._

Ths unique addition to the [cw20-stake] is that messages from this contract have an optional `relayed_from` argument,
which will accept forwarded message from an authorised proxy,
in the case of [VectisDAO], the dao-tunnel contract, the end of of all other chains IBC messages for the DAO.

[cw20-stake]: https://github.com/DA0-DA0/dao-contracts/tree/v1.0.0/contracts/cw20-stake
[vectisdao]: https://github.com/nymlab/vectis

This is a basic implementation of a cw20 staking contract. Staked
tokens can be unbonded with a configurable unbonding period. Staked
balances can be queried at any arbitrary height by external contracts.

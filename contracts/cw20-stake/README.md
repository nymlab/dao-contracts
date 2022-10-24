# Stake CW20 Vectis

_This is a fork of the original [cw20-stake] contract in the daodao v1 code._

Staked tokens can be unbonded with a configurable unbonding period. Staked balances can be queried at any arbitrary height by external contracts.

Ths unique addition to the [cw20-stake] is that messages from this contract have an optional `from` argument,
which will accept forwarded message from an authorised proxy,
in the case of [VectisDAO], the dao-tunnel contract, the end of of all other chains IBC messages for the DAO.

[cw20-stake]: https://github.com/DA0-DA0/dao-contracts/tree/v1.0.0/contracts/cw20-stake
[vectisdao]: https://github.com/nymlab/vectis

## Running this contract

You will need Rust 1.58.1+ with `wasm32-unknown-unknown` target installed.

You can run unit tests on this via:

`cargo test`

Once you are happy with the content, you can compile it to wasm via:

```
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp ../../target/wasm32-unknown-unknown/release/stake_cw20.wasm .
ls -l stake_cw20.wasm
sha256sum stake_cw20.wasm
```

Or for a production-ready (optimized) build, run a build command in the the repository root: https://github.com/CosmWasm/cw-plus#compiling.

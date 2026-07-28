# Namada Monitoring

Namada Monitoring is a tool designed to track and report various metrics related to the Namada blockchain. It provides insights into the blockchain's performance, validator activity, transaction statistics, and more.

## Features

- **Block Height Counter**: Tracks the latest block height of the Namada blockchain.
- **Block Time**: Tracks the time spent processing a block.
- **PoS Activity Metrics**: Measures the total amount of tokens bonded and unbonded.
- **Epoch Counter**: Tracks the latest epoch recorded on the blockchain.
- **Fees tracker**:  Total fees paid per token.
- **Validator Signatures counter**:  Tracks how many validators signed the latest block's commit, and how many were expected to.
- **Validator Slashes counter**:  Tracks the number of slashes in each block
- **Total Supply of Token**: Monitors the total and effective supply of tokens.
- **Transaction Metrics**: Tracks transaction activity, including batch sizes and transaction types.
- **Transfers amounts**: Tracks the total transfer amount per token and transfer kind (native, ibc, shielding, unshielding, shielded, mixed).
- **Validator State Metrics**: Number of validators in each proof-of-stake state.
- **Voting Power Metrics**: Tracks the number of validators required to reach 1/3 and 2/3 of the total voting power.

### A note on metric labels

Metrics describe the latest observed block, so they carry no `height` or `epoch`
label. Using either as a label creates one time series per block or per epoch,
which grows without bound and eventually overwhelms both the exporter and
Prometheus. Query the `block_height` and `epoch` metrics if you need to
correlate a value with when it was observed.

## Checks

Checks turn state into alerts, delivered to Slack, Telegram, or the log.

| Check | Alerts when |
|---|---|
| `halt` | No block for `chain.halt_threshold` seconds, or the windowed average block time exceeds `block_time * avg_block_time_factor` |
| `lag` | The monitor itself falls more than `chain.max_lag_blocks` behind the chain head |
| `block` | A single block took longer than `block_time * (1 + block_time_max_deviation)` |
| `pos` | Voting power is concentrated in too few validators, the consensus set shrinks by more than `pos.consensus_threshold`, bonds or unbonds jump, or too many commit votes are missed |
| `validator_state` | Validators are jailed, or drop out of the consensus set, at an epoch boundary |
| `slashes` | The block carries duplicate-vote or light-client-attack evidence |
| `governance` | A governance proposal is submitted, reporting its voting window |
| `tx` | Transactions exceed the configured section or batch limits |
| `fee` | Transactions pay a fee above the per-token threshold |
| `gas` | Transactions reserve far more gas than they use |
| `transfer_limit` | A transfer exceeds the per-token threshold |
| `ibc` | An IBC client is within `ibc.client_expiration_threshold` of expiring |
| `ibc_limit` | A token's supply approaches `ibc.supply_limit_threshold` of its IBC mint limit |
| `unknown_tx` | The monitor cannot decode a transaction kind, meaning it is under-reporting after a chain upgrade |

Every check runs by default. Turn one off in the `[checks]` table of
`config.toml`; see `config.toml.example`.

### A note on alert volume

Checks that inspect individual transactions report all of a block's matches in a
single alert rather than one message per transaction, and most then stay quiet
for a dampening window. A condition that holds for many consecutive blocks would
otherwise emit hundreds of messages an hour and get rate limited by Slack or
Telegram. Alerts that carry a security signal — large transfers, slashing
evidence, jailed validators — are aggregated but never dampened. The metrics
always carry exact per-block counts.

## Prerequisites

- Rust (stable and nightly toolchains)
- Docker (optional, for containerized deployment)
- Just (optional, for easier building)

## Installation

1. **Clone the repository**:
    ```sh
    git clone https://github.com/your-repo/namada-monitoring.git
    cd namada-monitoring
    ```

2. **Install Rust toolchains**:
    ```sh
    just devs
    ```

3. **Copy and modify config.toml.example**

4. **Run the project**:
    ```sh
    just run $CHAIN_ID $RPC 
    ```
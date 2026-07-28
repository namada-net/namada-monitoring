RUST_STABLE := "1.95.0"
RUST_NIGTHLY := "nightly"

devs:
    rustup toolchain install {{ RUST_STABLE }} --no-self-update --component clippy,rustfmt
    rustup toolchain install {{ RUST_NIGTHLY }} --no-self-update --component clippy,rustfmt

build:
    cargo build

run CHAIN_ID RPC CONFIG_PATH="config.toml":
    cargo run --release -- --chain-id {{CHAIN_ID}} --rpc {{RPC}} --config-path {{CONFIG_PATH}}

check:
    cargo check

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check
    
# CI installs the toolchain with actions-rust-lang/setup-rust-toolchain, which
# sets RUSTFLAGS=-D warnings. Deny them here too so a local run fails on the
# same lints CI does.
clippy:
    cargo clippy --all-targets -- -D warnings

clippy-fix:
    cargo clippy --all --fix --allow-dirty --allow-staged

clean:
    cargo clean

compose recipe="up":
    cd composer && just {{recipe}}
    #if [[ "w{{recipe}}" == "wup" ]]; then just build docker; fi


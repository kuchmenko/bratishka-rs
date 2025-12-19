# Show available commands
default:
    @just --list

# Run CLI (usage: just cli "https://youtube.com/...")
cli *ARGS:
    cargo run -p bratishka -- {{ARGS}}

# Run desktop app
desktop:
    cargo run --release -p bratishka-desktop

# Build release
build:
    cargo build --release

# Format + lint + build (run before commit)
check:
    cargo fmt
    cargo clippy -- -D warnings
    cargo build --release

# Install CLI to ~/.cargo/bin
install:
    cargo install --path crates/bratishka-cli

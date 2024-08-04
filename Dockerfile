FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y libssl-dev avahi-daemon libnotify-dev
WORKDIR /app
COPY --from=builder /app/target/release/elgato-keylight-cli /app/target/release/elgato-keylight-discover /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/elgato-keylight-cli"]

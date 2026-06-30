# syntax=docker/dockerfile:1
FROM rust:1.81-slim-bookworm AS chef
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef

FROM chef AS planner
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/syntrix-ai crates/syntrix-ai/
COPY apps/backend apps/backend/
RUN cargo chef prepare --recipe-path recipe.json --bin syntrix-backend

FROM chef AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin syntrix-backend

FROM gcr.io/distroless/cc-debian12:latest
COPY --from=builder /app/target/release/syntrix-backend /usr/local/bin/syntrix-backend
COPY --from=builder /app/apps/backend/migrations /migrations
EXPOSE 3001
CMD ["/usr/local/bin/syntrix-backend"]

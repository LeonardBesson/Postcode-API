FROM rust:1.40 as builder

RUN apt-get update && apt-get -y install ca-certificates cmake libssl-dev && rm -rf /var/lib/apt/lists/*

COPY . .

RUN rustup target add x86_64-unknown-linux-gnu
ENV PKG_CONFIG_ALLOW_CROSS=1
RUN cargo build --target x86_64-unknown-linux-gnu --release

FROM debian

# libpq is for diesel posgres: https://github.com/diesel-rs/diesel/blob/master/guide_drafts/backend_installation.md
RUN apt-get update && apt-get -y install ca-certificates libpq-dev
COPY --from=builder /target/x86_64-unknown-linux-gnu/release/postcode-service .

CMD ["/postcode-service"]

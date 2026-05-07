FROM rust:1.88-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build -p server --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/chronoseal

COPY --from=builder /app/target/release/server /usr/local/bin/chronoseal

EXPOSE 3000

ENV RUST_LOG=info

CMD ["chronoseal"]

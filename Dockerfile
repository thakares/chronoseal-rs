FROM rust:1.87-bookworm AS builder

WORKDIR /app

COPY . .

RUN cargo build -p chronoseal-server --bin chronoseal --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/chronoseal

COPY --from=builder /app/target/release/chronoseal /usr/local/bin/chronoseal
COPY frontend /usr/share/chronoseal/frontend

EXPOSE 3000

ENV RUST_LOG=info
ENV CHRONOSEAL_DB_PATH=/var/lib/chronoseal/chronoseal.sqlite
ENV CHRONOSEAL_FRONTEND_DIR=/usr/share/chronoseal/frontend
ENV CHRONOSEAL_PID_FILE=/run/chronoseal.pid

CMD ["chronoseal", "run"]

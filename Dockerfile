ARG RUST_IMAGE=rust:1.99.1-slim-bookworm
FROM ${RUST_IMAGE} AS build
WORKDIR /app/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
COPY backend/migrations ./migrations
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/backend/target/release/poketeam /usr/local/bin/poketeam
USER 65532:65532
EXPOSE 3000
ENTRYPOINT ["poketeam"]
CMD ["serve"]

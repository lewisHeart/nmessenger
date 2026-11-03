ARG RUST_VERSION=1.98
FROM rust:${RUST_VERSION}-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY nm-protocol ./nm-protocol
COPY nm-servo ./nm-servo
COPY nmessenger-client ./nmessenger-client
RUN cargo build --locked --release -p nm-servo && \
    install -m 755 target/release/nm-servo /usr/local/bin/nm-servo

FROM debian:bookworm-slim AS runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid 10001 nmessenger && \
    useradd --uid 10001 --gid 10001 --no-create-home nmessenger && \
    install -d -m 750 -o 10001 -g 10001 /app/data/uploads
WORKDIR /app
COPY --from=build /usr/local/bin/nm-servo /usr/local/bin/nm-servo
USER 10001:10001
ENV BIND_ADDR=0.0.0.0:8080 UPLOAD_DIR=/app/data/uploads
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/nm-servo"]

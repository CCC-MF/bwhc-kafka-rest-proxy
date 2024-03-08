FROM rust:alpine AS back-stage

RUN apk update
RUN apk add cmake make musl-dev g++

WORKDIR /build
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

# Build image from scratch
FROM scratch
LABEL org.opencontainers.image.source="https://github.com/CCC-MF/bwhc-kafka-rest-proxy"
LABEL org.opencontainers.image.licenses="AGPL-3.0-or-later"
LABEL org.opencontainers.image.description="bwHC MTB-File REST Proxy für Kafka"

COPY --from=back-stage /build/target/release/bwhc-kafka-rest-proxy .
USER 65532
EXPOSE 3000
CMD ["./bwhc-kafka-rest-proxy"]
ARG IMAGE_ARCH

FROM docker.io/library/rust:1 AS build
ARG RUST_ARCH
RUN rustup target add ${RUST_ARCH}
COPY . /app
RUN cargo build \
    --manifest-path=/app/Cargo.toml \
    --target=${RUST_ARCH} \
    --release \
    --verbose && \
    cp /app/target/${RUST_ARCH}/release/fernbedienung /fernbedienung

FROM docker.io/${IMAGE_ARCH}/alpine:3
COPY --from=build /fernbedienung /usr/local/bin/
CMD ["/usr/local/bin/fernbedienung"]
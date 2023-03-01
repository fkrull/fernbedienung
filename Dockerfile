FROM --platform=$BUILDPLATFORM docker.io/library/rust:1 AS build
ARG TARGET
RUN rustup target add ${TARGET}
COPY . /app
RUN cargo build \
    --manifest-path=/app/Cargo.toml \
    --target=${TARGET} \
    --release \
    --verbose && \
    cp /app/target/${TARGET}/release/fernbedienung /fernbedienung

FROM --platform=$TARGETPLATFORM docker.io/library/alpine:3.17.2
RUN apk add --no-cache mpc
COPY --from=build /fernbedienung /usr/local/bin/
CMD ["/usr/local/bin/fernbedienung"]

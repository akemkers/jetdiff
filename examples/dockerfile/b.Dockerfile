# Build the server and run it in a small image
FROM rust:1.82 AS build
ARG PROFILE=release
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --profile "$PROFILE"

FROM debian:bookworm-slim
RUN useradd --system server
USER server
COPY --from=build /src/target/release/server /usr/local/bin/server
ENV RUST_LOG=info
EXPOSE 8080 9090
HEALTHCHECK CMD ["server", "--check"]
CMD ["server", "--port", "8080"]

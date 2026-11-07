# Build the server and run it in a small image
FROM rust:1.80 AS build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/server /usr/local/bin/server
EXPOSE 8080
CMD ["server"]

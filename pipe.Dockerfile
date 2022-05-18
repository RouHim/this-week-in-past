# Using the `rust-musl-builder` as base image, instead of the official Rust toolchain
FROM clux/muslrust:stable AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Planner
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Notice that we are specifying the --target flag to build with musl!
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# The actual image to run
FROM scratch AS runtime
ENV RESOURCE_PATHS=/resources
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/this-week-in-past /app
COPY --from=builder /app/static /static
VOLUME /cache
EXPOSE 8080
USER 1337
CMD ["/app"]
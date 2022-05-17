ARG BASE_IMAGE=rust:slim-buster

FROM $BASE_IMAGE as planner
WORKDIR app
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM $BASE_IMAGE as cacher
WORKDIR app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM $BASE_IMAGE as builder
WORKDIR app
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /app/target target
COPY --from=cacher $CARGO_HOME $CARGO_HOME
# We need static linking for musl
RUN rustup target add x86_64-unknown-linux-musl
# `cargo build` doesn't work in static linking, need `cargo install`
RUN cargo install --target x86_64-unknown-linux-musl --path .

FROM scratch
COPY --from=builder /usr/local/cargo/bin/this-week-in-past .
CMD ["./this-week-in-past"]
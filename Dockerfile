FROM rust as build
WORKDIR /tailor
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install --assume-yes musl-tools
COPY . .
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --target=x86_64-unknown-linux-musl --release


FROM scratch
WORKDIR /opt/tailor/bin
EXPOSE 8080
ENTRYPOINT ["./tailor"]
COPY --from=build tailor/target/x86_64-unknown-linux-musl/release/tailor .
COPY --from=build tailor/assets assets

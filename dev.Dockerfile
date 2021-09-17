FROM rust:1.54-buster

WORKDIR /opt/webhook

# Dependencies
RUN apt-get update
RUN apt-get install ca-certificates -y

CMD cargo build --package webhook --bin webhook && exec ./target/debug/webhook

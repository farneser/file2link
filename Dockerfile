FROM rust:latest as build

RUN apt-get update && apt-get install -y \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .

RUN cargo build --release

FROM ubuntu as run

COPY --from=build /app/target/release/file2link /app/file2link

RUN chmod +x /app/file2link

ENV SERVER_PORT ${SERVER_PORT}

EXPOSE ${SERVER_PORT}

CMD ["/app/file2link"]

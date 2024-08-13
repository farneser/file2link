FROM rust:latest as build

RUN apt-get update && apt-get install -y \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY . .

RUN cargo build --release

FROM ubuntu as run

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=build /build/target/release/file2link /app/file2link
COPY --from=build /build/target/release/f2l-cli /app/f2l-cli

RUN chmod +x /app/file2link
RUN chmod +x /app/f2l-cli

ENV PATH="/app:${PATH}"

ENV F2L_PIPE_PATH "/app/f2l.pipe"
ENV SERVER_PORT ${SERVER_PORT}

EXPOSE ${SERVER_PORT}

CMD ["/app/file2link"]

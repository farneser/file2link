FROM rust:latest as build

RUN apt-get update && apt-get install -y \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .

RUN cargo build --release

FROM alpine:latest as run

COPY --from=build /app/target/release/file2link /app/file2link

RUN chmod +x /file2link

ENV PORT=8080

EXPOSE ${PORT}

CMD ["/file2link"]

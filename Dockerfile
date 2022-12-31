FROM rust:1.65 AS builder
WORKDIR /app
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
RUN apt-get update -y && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/static-video-server /usr/local/bin/static-video-server

ENTRYPOINT ["static-video-server"]
CMD ["--assets-root", "/assets", "--port", "9092", "--host", "0.0.0.0"]

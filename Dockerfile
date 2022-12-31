FROM rust:1.65 AS builder
WORKDIR /app
COPY . .
RUN cargo install --path .

EXPOSE 80

ENTRYPOINT ["static-video-server"]
CMD ["--assets-root", "/videos", "--port", "80", "--host", "0.0.0.0"]

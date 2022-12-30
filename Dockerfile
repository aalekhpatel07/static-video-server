FROM rust:1.65
WORKDIR /app
COPY . .
RUN cargo install --path .
ENTRYPOINT ["static-video-server"]
CMD ["--assets-root", "/assets", "--port", "9092", "--host", "0.0.0.0"]

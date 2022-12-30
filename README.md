# Static Video Server

Host a collection of static video files nested arbitrarily deep in a root directory.

## Usage

The most common usage would be to provide an index of all available videos inside a root directory.

For example, host all the videos present (arbitrarily deep) inside `~/Videos` via a static file server
available at `localhost:9092`.

### Natively:

```sh
$ cargo install static-video-server
```

```sh
$ RUST_LOG="info" static-video-server --assets-root "~/Videos" --port 9092 --host "0.0.0.0"
```

### Docker

```sh
# Map your content root directory to container's /assets and bind ports 9092
# to access the web UI from host.

$ docker run -d --rm -v ~/Videos:/assets -p 9092:9092 static-video-server:latest
```

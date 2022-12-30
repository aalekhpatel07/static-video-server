# Static Video Server

Host a collection of static video files nested arbitrarily deep in a root directory.

## Installation

### Using Cargo
```sh
$ cargo install static-video-server
```

## Usage

The most common usage would be to provide an index of all available videos inside a root directory.

To host all the videos present (arbitrarily deep) inside `~/Videos` via a static file server
available at `localhost:9092`:

```sh
$ RUST_LOG="info" static-video-server --assets-root "~/Videos" --port 9092 --host "0.0.0.0"
```

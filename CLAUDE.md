# Rust JMAP Webmail

A minimalist webmail client using Rust, htmx, and JMAP.

## Build

```bash
export CC=gcc
cargo build
```

CC is required because `ring` (TLS crypto) contains C code.

## Run

```bash
./target/debug/rust-jmap-webmail
# Listens on http://127.0.0.1:8080
```

## Configuration

Edit `config.toml`:
- `server.listen_addr` / `server.listen_port` - HTTP server binding
- `jmap.well_known_url` - JMAP server discovery URL

## Architecture

- **No async runtime** - uses blocking I/O (`tiny_http`, `ureq`)
- **htmx** - dynamic UI updates without custom JS
- **Sessions** - UUIDv7 cookies, credentials stored in-memory
- **Templates** - server-side HTML generation in `src/templates/`

## Known Issues

- JMAP authentication not working yet - redirect handling for `.well-known/jmap` may still have issues with the Authorization header

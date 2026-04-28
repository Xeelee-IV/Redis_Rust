# Redis Clone in Rust

A working Redis server built from scratch in Rust, following the [CodeCrafters "Build Your Own Redis"](https://codecrafters.io/challenges/redis) challenge. This project implements the core Redis wire protocol (RESP) and a growing set of commands using async networking with Tokio.

---

## What it does

This server binds to port `6379` — the same port as real Redis — and speaks the same protocol, meaning you can point `redis-cli` at it and it just works.

```bash
$ redis-cli PING
PONG

$ redis-cli SET name Alice
OK

$ redis-cli GET name
"Alice"

$ redis-cli SET token abc123 PX 100
OK

$ redis-cli RPUSH mylist a b c
(integer) 3
```

---

## Implemented commands

| Command | Syntax | Description |
|---|---|---|
| `PING` | `PING` | Returns `PONG` |
| `ECHO` | `ECHO <message>` | Returns the message back |
| `SET` | `SET <key> <value> [EX seconds] [PX milliseconds]` | Stores a key with optional expiry |
| `GET` | `GET <key>` | Retrieves a value, returns null if missing or expired |
| `RPUSH` | `RPUSH <key> <value> [value ...]` | Appends one or more elements to a list |

---

## How it works

### Architecture

```
Client (redis-cli)
       │
       │  TCP on port 6379
       ▼
  main() — TcpListener accepts connections
       │
       │  tokio::task::spawn (one task per client)
       ▼
  handle_connection() — reads bytes in a loop
       │
       ▼
  parse_resp() — decodes the RESP wire protocol
       │
       ▼
  handle_command() — dispatches and builds response
       │
       ▼
  socket.write_all() — sends bytes back to client
```

### Concurrency

Each client connection gets its own async Tokio task. The shared key-value store is protected by an `Arc<Mutex<HashMap>>` — `Arc` gives every task a pointer to the same data, and `Mutex` ensures only one task accesses it at a time.

### RESP Protocol

All communication uses the Redis Serialization Protocol (RESP). For example, `SET name Alice` is sent over the wire as:

```
*3\r\n$3\r\nSET\r\n$4\r\nname\r\n$5\r\nAlice\r\n
```

The parser handles this format entirely from scratch with no external libraries.

### Data model

Each stored entry holds a value and an optional expiry timestamp:

```rust
enum RedisValue {
    String(String),
    List(Vec<String>),
}

struct Entry {
    value: RedisValue,
    expires_at: Option<Instant>,
}
```

Expiry uses **lazy deletion** — expired keys are checked and removed at the moment they are accessed, not on a background timer. This is how real Redis handles basic expiry.

---

## Getting started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable, 2021 edition or later)
- Tokio (included via `Cargo.toml`)

### Run the server

```bash
git clone https://github.com/Xeelee-IV/redis-rust
cd redis-rust
cargo run
```

The server starts on `127.0.0.1:6379`.

### Test it

With `redis-cli` installed:

```bash
redis-cli PING
redis-cli SET foo bar PX 5000
redis-cli GET foo
redis-cli RPUSH mylist one two three
```

Or with `nc` (netcat) if you want to speak raw RESP:

```bash
echo -e "*1\r\n\$4\r\nPING\r\n" | nc 127.0.0.1 6379
```

---

## Project structure

```
src/
└── main.rs        # Everything: server, parser, command handler, data store
```

---

## What I learned building this

- **Async Rust with Tokio** — how `async`/`await`, green threads, and the Tokio runtime work
- **TCP networking** — binding ports, accepting connections, reading byte streams
- **The RESP protocol** — parsing a real binary protocol without libraries
- **Rust ownership** — why `Arc<Mutex<T>>` is the right pattern for shared state across async tasks, and how the borrow checker enforces safe concurrency
- **Rust enums for type modelling** — using `enum RedisValue` to cleanly represent different Redis value types (strings, lists) in the same store
- **Lazy expiry** — how Redis handles TTL without background timers

---

## Roadmap

Stages still to implement from the CodeCrafters track:

- [ ] `LRANGE` — read elements from a list
- [ ] RDB persistence — save and load data from disk
- [ ] Redis replication — leader/replica setup
- [ ] Redis Streams

---

## Resources

- [CodeCrafters — Build Your Own Redis](https://codecrafters.io/challenges/redis)
- [Redis Protocol Specification (RESP)](https://redis.io/docs/latest/develop/reference/protocol-spec/)
- [Tokio documentation](https://tokio.rs)
- [The Rust Book](https://doc.rust-lang.org/book/)

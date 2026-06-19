# cex-v1

`cex-v1` is an early work-in-progress centralized spot market exchange service written in Rust.

The goal of this project is to learn Rust by building the core pieces of a CEX-style matching engine from the ground up. It is not production-ready, and a lot of the behavior is still being explored, changed, and improved as the implementation grows.

## What This Service Does

The service runs as a queue-driven exchange engine:

- listens for JSON requests from a Redis request queue
- reads the response queue name from each request payload
- processes the request through the exchange engine
- sends the JSON response back to the requested Redis response queue
- keeps exchange state in memory for fast reads and writes

The in-memory store currently tracks:

- user balances
- order books
- order records
- fills

At the moment the store is initialized with `SOL` and `BTC` order books, using `INR` as the primary currency.

## Current Scope

The request model currently includes:

- initializing a user's balance
- creating an order
- getting market depth
- getting a user balance
- getting an order
- cancelling an order

The matching engine work is still in progress. The code currently focuses mostly on limit buy order flow, balance updates, order book state, and fill tracking. Some request types and engine paths are placeholders or incomplete while I continue building out the full behavior.

## How It Runs

Configuration is read from environment variables:

```sh
REDIS_URL=redis://127.0.0.1/
REDIS_REQUEST_QUEUE=exchange-requests
REDIS_BRPOP_TIMEOUT_SECONDS=5
```

`REDIS_BRPOP_TIMEOUT_SECONDS` is optional and defaults to `5`.

Run the service with:

```sh
cargo run
```

## Project Status

This repository is a learning project (so that later I can build a perps engine) and a work in progress.

I am using it to get better at Rust while trying to implement the core ideas behind a centralized exchange matching engine.

Until next time...

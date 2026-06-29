# AI-DDS

AI-DDS is a Rust implementation of the Data Distribution Service (DDS) middleware, providing a flexible, high‑performance publish‑subscribe framework. The repository includes examples demonstrating basic pub/sub, security with certificates, QoS policies, and multi‑process configurations.

## Features

- Core DDS functionality with support for topics, publishers, subscribers, and data writers/readers.
- Security plugin using certificates stored on disk.
- QoS examples covering reliability, history, and durability.
- Simple examples for single‑process and multi‑process scenarios.

## Getting Started

```bash
# Build all examples
cargo build --examples

# Run an example (e.g., hello_world)
cargo run -p dds --example hello_world -- pub
```

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.

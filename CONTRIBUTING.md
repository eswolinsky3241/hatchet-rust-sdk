# Contributing to Hatchet Rust SDK

Thank you for your interest in contributing to the Hatchet Rust SDK! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- Rust 1.75+ (2024 edition)
- Protocol Buffers compiler (`protoc`)
- Docker (for integration tests)

### Setting Up Your Development Environment

1. **Clone the repository:**
   ```bash
   git clone https://github.com/eswolinsky3241/hatchet-rust-sdk.git
   cd hatchet-rust-sdk
   ```

2. **Install Protocol Buffers compiler:**
   ```bash
   # On macOS
   brew install protobuf
   
   # On Ubuntu/Debian
   sudo apt-get install protobuf-compiler
   
   # On other systems, download from:
   # https://github.com/protocolbuffers/protobuf/releases
   ```

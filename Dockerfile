# Build Stage
FROM rust:1.81-slim-bullseye AS builder

WORKDIR /app
COPY . .

# Install dependencies for building
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Build the application
RUN cargo build --release

# Runtime Stage
FROM debian:bullseye-slim

WORKDIR /app

# Install runtime dependencies (SSL)
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/pryect /app/sentinel
COPY --from=builder /app/www /app/www

# Expose the proxy port
EXPOSE 3000

# Run the firewall
CMD ["./sentinel"]

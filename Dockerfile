# ========================
# Build Stage
# ========================
FROM rust:1.82.0-alpine3.20 AS builder

# Install build dependencies, including static OpenSSL libraries
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    build-base \
    curl \
    git

# Set environment variables for static linking with OpenSSL
ENV OPENSSL_STATIC=yes
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include

# Add the MUSL target for static linking
RUN rustup target add x86_64-unknown-linux-musl

# Set the working directory
WORKDIR /usr/src/app

# Copy over Cargo.toml and Cargo.lock for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy over all the source code
COPY . .

# Build the project in release mode for the MUSL target
RUN cargo build --release --bin service --bin watcher --target x86_64-unknown-linux-musl

# Strip the binaries to reduce size
RUN strip target/x86_64-unknown-linux-musl/release/service
RUN strip target/x86_64-unknown-linux-musl/release/watcher

# ========================
# Runtime Stage
# ========================
FROM alpine:3.20

# Install runtime dependencies (only ca-certificates)
RUN apk add --no-cache ca-certificates

# Copy the compiled binaries from the builder stage
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/service /usr/local/bin/service
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/watcher /usr/local/bin/watcher

# Set the working directory
WORKDIR /usr/local/bin

# Expose the port the service listens on
EXPOSE 8080

# Set the default command to run the service binary
CMD ["service"]

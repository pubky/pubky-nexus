# ========================
# Build Stage
# ========================
FROM rust:1.87.0-alpine3.22 AS builder

# Build platform argument (x86_64 or aarch64) (default: x86_64)
ARG TARGETARCH=x86_64
RUN echo "TARGETARCH: $TARGETARCH"

# Install build dependencies, including static OpenSSL libraries
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    build-base \
    curl \
    git

# Install cross-compiler toolchain only for ARM (Apple Silicon)
RUN if [ "$TARGETARCH" = "aarch64" ]; then \
    wget -qO- https://musl.cc/aarch64-linux-musl-cross.tgz | tar -xz -C /usr/local && \
    echo "/usr/local/aarch64-linux-musl-cross/bin" > /tmp/musl_cross_path; \
    else \
    echo "" > /tmp/musl_cross_path; \
    fi

# Set PATH only if we installed the cross compiler (will be empty string for x86)
ENV PATH="$(cat /tmp/musl_cross_path):$PATH"

# Set environment variables for static linking with OpenSSL
ENV OPENSSL_STATIC=yes
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include

# Add the MUSL target for static linking
RUN rustup target add $TARGETARCH-unknown-linux-musl

# Set the working directory
WORKDIR /usr/src/app

# Copy over Cargo.toml and Cargo.lock for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy over all the source code
COPY . .

# Build the project in release mode for the MUSL target
RUN cargo build --release --bin nexusd --target $TARGETARCH-unknown-linux-musl

# Strip the binaries to reduce size
RUN strip target/$TARGETARCH-unknown-linux-musl/release/nexusd

# ========================
# Runtime Stage
# ========================
FROM alpine:3.22

ARG TARGETARCH=x86_64

# Install runtime dependencies (only ca-certificates)
RUN apk add --no-cache ca-certificates

# Copy the compiled binaries from the builder stage
COPY --from=builder /usr/src/app/target/$TARGETARCH-unknown-linux-musl/release/nexusd /usr/local/bin/nexusd

# Set the working directory
WORKDIR /usr/local/bin

# Expose the port the service listens on
EXPOSE 8080

# Set the default command to run the service binary
CMD ["nexusd"]

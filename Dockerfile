# ========================
# Build Stage
# ========================
FROM rust:1.90.0-alpine3.22 AS builder

# Build platform argument (x86_64 or aarch64) (default: x86_64)
ARG TARGETARCH
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

# Set PATH only if we installed the cross compiler (will be empty string for x86)
ENV PATH="$(cat /tmp/musl_cross_path):$PATH"

# Set environment variables for static linking with OpenSSL
ENV OPENSSL_STATIC=yes
ENV OPENSSL_LIB_DIR=/usr/lib
ENV OPENSSL_INCLUDE_DIR=/usr/include

# Set the working directory
WORKDIR /usr/src/app

# Copy over Cargo.toml and Cargo.lock for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy over all the source code
COPY . .

# Build the project in release mode for the MUSL target
RUN cargo build --release --bin nexusd

# Strip the binaries to reduce size
RUN strip target/release/nexusd

# ========================
# Runtime Stage
# ========================
FROM alpine:3.22

ARG TARGETARCH

# Install runtime dependencies
RUN apk add --no-cache ca-certificates \
    imagemagick \
    imagemagick-webp \
    imagemagick-heic \
    imagemagick-svg \
    imagemagick-jpeg \
    imagemagick-tiff \
    imagemagick-raw

# Copy the compiled binaries from the builder stage
COPY --from=builder /usr/src/app/target/release/nexusd /usr/local/bin/nexusd

# Set the working directory
WORKDIR /usr/local/bin

# Expose the port the service listens on
EXPOSE 8080

# Set the default command to run the service binary
CMD ["nexusd"]

# ------------------------------------------------------------------------------
# Build Stage
# ------------------------------------------------------------------------------
FROM --platform=$BUILDPLATFORM rust:1-bullseye as builder
ARG BUILDPLATFORM
ARG TARGETPLATFORM
ARG CARGO_PROFILE=release

RUN echo "DEBIAN_ARCH_TRIPLET='$(dpkg-architecture -A${TARGETPLATFORM#linux/} -qDEB_TARGET_MULTIARCH)'" >> /root/dynenv
RUN . /root/dynenv && \
  echo "RUST_ARCH_TRIPLET='$(echo "$DEBIAN_ARCH_TRIPLET" | sed -E 's/-linux-/-unknown&/')'" >> /root/dynenv

WORKDIR /build

# Copy source tree
COPY . .

# Install common dependencies
RUN apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y \
  clang \
  cmake \
  protobuf-compiler \
  git \
  curl \
  libssl-dev \
  build-essential \
  gcc-multilib \
  g++-multilib

# Add support for cross-compilation
RUN dpkg --add-architecture armhf && \
  dpkg --add-architecture arm64 && \
  apt-get update && apt-get install -y \
  gcc-arm-linux-gnueabihf \
  g++-arm-linux-gnueabihf \
  gcc-aarch64-linux-gnu \
  g++-aarch64-linux-gnu \
  libssl-dev:armhf \
  libssl-dev:arm64

# Configure cross-build environment
RUN set -x && \
  if [ "$TARGETPLATFORM" != "$BUILDPLATFORM" ]; then \
  . /root/dynenv && \
  apt install -y gcc-$DEBIAN_ARCH_TRIPLET binutils-$DEBIAN_ARCH_TRIPLET && \
  rustup target add "$RUST_ARCH_TRIPLET" && \
  echo "RUSTFLAGS='-C linker=$DEBIAN_ARCH_TRIPLET-gcc'; export RUSTFLAGS" >> /root/dynenv; \
  fi

# Build
RUN set -x && \
  . /root/dynenv && \
  cargo build --locked $CARGO_OPTIONS --profile $CARGO_PROFILE --target "$RUST_ARCH_TRIPLET" && \
  mkdir -p build/bin && \
  mv target/$RUST_ARCH_TRIPLET/$CARGO_PROFILE/cord /build/

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12 as runtime
LABEL maintainer="engineering@dhiway.com"
LABEL description="Cord: a platform for digital trust."

# Create cord user
USER 1000:1000

# Install
COPY --from=builder /build/cord /cord

# Configuration
EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/cord"]

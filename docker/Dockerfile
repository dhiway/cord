# Workaround for https://github.com/containers/buildah/issues/4742
FROM rust:1-bullseye as target

LABEL maintainer="engineering@dhiway.com"

# Build Stage
FROM --platform=$BUILDPLATFORM rust:1-bullseye as build
ARG BUILDPLATFORM
ARG TARGETPLATFORM
ARG PROFILE

RUN echo "DEBIAN_ARCH_TRIPLET='$(dpkg-architecture -A${TARGETPLATFORM#linux/} -qDEB_TARGET_MULTIARCH)'" >>/root/dynenv
RUN . /root/dynenv && \
    echo "RUST_ARCH_TRIPLET='$(echo "$DEBIAN_ARCH_TRIPLET" | sed -E 's/-linux-/-unknown&/')'" >>/root/dynenv

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
    build-essential

# Add support for cross-compilation
RUN dpkg --add-architecture armhf && \
    apt-get update && apt-get install -y \
    gcc-arm-linux-gnueabihf \
    g++-arm-linux-gnueabihf \
    libssl-dev:arm-linux-gnueabihf

# build cord
ARG debug=0
RUN if [ "$debug" = 0 ]; then \
        echo "CARGO_OPTIONS=--release" >>/root/dynenv && \
        echo "TARGET_FOLDER=release" >>/root/dynenv; \
    else \
        echo "TARGET_FOLDER=debug" >>/root/dynenv; \
    fi

# Configure cross-build environment if need be
RUN set -x && \
    if [ "$TARGETPLATFORM" != "$BUILDPLATFORM" ]; then \
        . /root/dynenv && \
        apt install -y gcc-$DEBIAN_ARCH_TRIPLET binutils-$DEBIAN_ARCH_TRIPLET && \
        rustup target add "$RUST_ARCH_TRIPLET" && \
        echo "RUSTFLAGS='-C linker=$DEBIAN_ARCH_TRIPLET-gcc'; export RUSTFLAGS" >>/root/dynenv; \
    fi

# Build
ARG chain="cord"
RUN set -x && \
    cat /root/dynenv && \
    . /root/dynenv && \
    cargo build --locked $CARGO_OPTIONS $PROFILE --target "$RUST_ARCH_TRIPLET" && \
    mkdir -p build && \
    mv target/$RUST_ARCH_TRIPLET/$TARGET_FOLDER/cord build/ && \

# Conditional installation of cross-compilation tools
RUN if [ "$target" = "aarch64-unknown-linux-gnu" ]; then \
  apt-get install -y \
    gcc-aarch64-linux-gnu \
    binutils-aarch64-linux-gnu \
    g++-aarch64-linux-gnu && \
  echo "export CC=aarch64-linux-gnu-gcc" >> /build/dynenv && \
  echo "export CXX=aarch64-linux-gnu-g++" >> /build/dynenv; \
  fi

#
# Final Stage
#
FROM debian:bullseye-slim
LABEL maintainer="engineering@dhiway.com"
LABEL description="CORD Blockchain Node"

# Create cord user
RUN useradd -m -d /data -u 1000 cord

# Install
COPY --from=builder /root/build/cord /usr/local/bin

# unclutter and minimize the attack surface
# check if executable works in this container
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    apt-get clean && rm -rf /var/lib/apt/lists/* && \
    rm -rf /usr/bin /usr/sbin && \
    /usr/local/bin/cord --version

# Configuration
# p2p, telemetry, rpc, prometheus
EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]
USER cord

ENTRYPOINT ["/usr/local/bin/cord"]

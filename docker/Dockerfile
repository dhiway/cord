# Build Stage
FROM rust:1-bullseye as builder

LABEL maintainer="engineering@dhiway.com"

ARG target
ARG profile=release

WORKDIR /build

# Install common dependencies
RUN apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y clang cmake protobuf-compiler git curl libssl-dev build-essential

# Conditional installation of cross-compilation tools
RUN if [ "$target" = "linux/arm64" ] || [ "$target" = "linux/arm64/v8" ]; then \
  apt-get install -y gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu g++-aarch64-linux-gnu && \
  echo "export CC=aarch64-linux-gnu-gcc" >> /build/dynenv && \
  echo "export CXX=aarch64-linux-gnu-g++" >> /build/dynenv; \
  fi

# Copy source tree
COPY . /build

# Configure build environment and build
RUN set -x && \
  if [ -f /build/dynenv ]; then . /build/dynenv; fi && \
  cargo build --locked --profile $profile && \
  mkdir -p bin && \
  mv target/$target/$profile/cord bin/

#
# Final Stage
#
FROM debian:bullseye-slim
LABEL maintainer="engineering@dhiway.com"
LABEL description="CORD Blockchain Node"

VOLUME ["/data"]

COPY --from=builder /build/bin/cord /usr/local/bin

# unclutter and minimize the attack surface
# check if executable works in this container
RUN rm -rf /usr/bin /usr/sbin && \
  /usr/local/bin/cord --version

USER 1000:1000
EXPOSE 30333 9933 9944 9615

ENTRYPOINT ["/usr/local/bin/cord"]

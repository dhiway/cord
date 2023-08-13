# ------------------------------------------------------------------------------
# Build Stage
# ------------------------------------------------------------------------------

FROM rust:1-bullseye as build

LABEL maintainer="engineering@dhiway.com"

ARG target
ARG profile=release

WORKDIR /builder

# Install common dependencies
RUN apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y clang cmake protobuf-compiler git curl libssl-dev build-essential

# Conditional installation of cross-compilation tools
RUN if [ "$target" = "linux/arm64" ] || [ "$target" = "linux/arm64/v8" ]; then \
  apt-get install -y gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu g++-aarch64-linux-gnu && \
  echo "export CC=aarch64-linux-gnu-gcc" >> /builder/dynenv && \
  echo "export CXX=aarch64-linux-gnu-g++" >> /builder/dynenv; \
  fi

# Copy source tree
COPY . /builder

# Configure build environment and build
RUN set -x && \
  if [ -f /builder/dynenv ]; then . /builder/dynenv; fi && \
  cargo build --locked --profile $profile && \
  mkdir -p build && \
  mv target/$target/$profile/cord build/

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------

FROM debian:bullseye-slim
LABEL maintainer="engineering@dhiway.com"
LABEL description="CORD Blockchain Node"

COPY --from=build /builder/build/cord /cord

RUN ["/cord","--version"]

USER 1000:1000
EXPOSE 30333 9933 9944 9615

ENTRYPOINT ["/cord"]

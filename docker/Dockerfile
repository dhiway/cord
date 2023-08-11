# ------------------------------------------------------------------------------
# Build Stage
# ------------------------------------------------------------------------------

# When building for a foreign arch, use cross-compilation
# https://www.docker.com/blog/faster-multi-platform-builds-dockerfile-cross-compilation-guide/
FROM --platform=$BUILDPLATFORM rust:1-bullseye as build

LABEL maintainer="engineering@dhiway.com"

ARG BUILDPLATFORM
ARG TARGETPLATFORM

# We need the target arch triplet in both Debian and rust flavor
RUN echo "DEBIAN_ARCH_TRIPLET='$(dpkg-architecture -A${TARGETPLATFORM#linux/} -qDEB_TARGET_MULTIARCH)'" >>/builder/dynenv
RUN . /builder/dynenv && \
  echo "RUST_ARCH_TRIPLET='$(echo "$DEBIAN_ARCH_TRIPLET" | sed -E 's/-linux-/-unknown&/')'" >>/builder/dynenv

WORKDIR /builder

# Copy source tree
COPY . .

RUN apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y clang cmake protobuf-compiler

# build cord
ARG debug=0
RUN if [ "$debug" = 0 ]; then \
  echo "CARGO_OPTIONS=--production" >>/builder/dynenv && \
  echo "TARGET_FOLDER=production" >>/builder/dynenv; \
  else \
  echo "TARGET_FOLDER=debug" >>/builder/dynenv; \
  fi

# Configure cross-build environment if need be
RUN set -x && \
  if [ "$TARGETPLATFORM" != "$BUILDPLATFORM" ]; then \
  . /builder/dynenv && \
  apt install -y gcc-$DEBIAN_ARCH_TRIPLET binutils-$DEBIAN_ARCH_TRIPLET && \
  rustup target add "$RUST_ARCH_TRIPLET" && \
  echo "RUSTFLAGS='-C linker=$DEBIAN_ARCH_TRIPLET-gcc'; export RUSTFLAGS" >>/builder/dynenv; \
  fi

# Build
RUN set -x && \
  cat /builder/dynenv && \
  . /builder/dynenv && \
  cargo build --locked $CARGO_OPTIONS --target "$RUST_ARCH_TRIPLET" && \
  mkdir -p build && \
  mv target/$RUST_ARCH_TRIPLET/$TARGET_FOLDER/cord build/

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------

FROM gcr.io/distroless/cc-debian11@sha256:9b8e0854865dcaf49470b4ec305df45957020fbcf17b71eeb50ffd3bc5bf885d
LABEL maintainer="engineering@dhiway.com"
LABEL description="CORD Blockchain Node"

COPY --from=builder /builder/build/cord /cord

RUN ["/cord","--version"]

USER 1000:1000
EXPOSE 30333 9933 9944 9615

ENTRYPOINT ["/cord"]

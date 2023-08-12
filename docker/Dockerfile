# ------------------------------------------------------------------------------
# Build Stage
# ------------------------------------------------------------------------------

FROM rust:1-bullseye as build

LABEL maintainer="engineering@dhiway.com"

ARG TARGETPLATFORM

WORKDIR /builder

# Conditional installation of cross-compilation tools and setting compiler flags
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ] || [ "$TARGETPLATFORM" = "linux/arm64/v8" ]; then \
  apt-get update && \
  apt-get install -y gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu g++-aarch64-linux-gnu && \
  echo "export CC=aarch64-linux-gnu-gcc" >> /builder/dynenv && \
  echo "export CXX=aarch64-linux-gnu-g++" >> /builder/dynenv; \
  elif [ "$TARGETPLATFORM" = "linux/amd64" ]; then \
  apt-get update && \
  apt-get install -y build-essential; \
  fi

# Copy source tree
COPY . /builder

# Update package lists and install required dependencies
RUN apt-get update && \
  DEBIAN_FRONTEND=noninteractive apt-get install -y clang cmake protobuf-compiler git curl libssl-dev 

# build cord
ARG debug=0
RUN if [ "$debug" = 0 ]; then \
  echo "CARGO_OPTIONS=production" >>/builder/dynenv && \
  echo "TARGET_FOLDER=production" >>/builder/dynenv; \
  else \
  echo "TARGET_FOLDER=debug" >>/builder/dynenv; \
  fi

# Configure build environment and build
RUN set -x && \
  . /builder/dynenv && \
  cargo build --locked --profile $CARGO_OPTIONS && \
  mkdir -p build && \
  mv target/$TARGETPLATFORM/$TARGET_FOLDER/cord build/

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------

FROM gcr.io/distroless/cc-debian11@sha256:9b8e0854865dcaf49470b4ec305df45957020fbcf17b71eeb50ffd3bc5bf885d
LABEL maintainer="engineering@dhiway.com"
LABEL description="CORD Blockchain Node"

COPY --from=build /builder/build/cord /cord

RUN ["/cord","--version"]

USER 1000:1000
EXPOSE 30333 9933 9944 9615

ENTRYPOINT ["/cord"]

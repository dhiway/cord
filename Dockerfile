# This is the build stage for CORD. Here we create the binary in a temporary image.
FROM docker.io/paritytech/ci-linux:production as builder

LABEL maintainer="engineering@dhiway.com"
ARG PROFILE=production

WORKDIR /build
COPY . /build

RUN cargo build --locked --profile ${PROFILE}

# test
# RUN cargo test --release --all

# This is the 2nd stage: a very small image where we copy the CORD binary."
FROM gcr.io/distroless/cc-debian11@sha256:9b8e0854865dcaf49470b4ec305df45957020fbcf17b71eeb50ffd3bc5bf885d
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=production

COPY --from=builder /build/target/${PROFILE}/cord /cord

RUN ["/cord","--version"]

USER 1000:1000
EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/cord","-d","/data"]

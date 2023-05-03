# This is the build stage for CORD. Here we create the binary in a temporary image.
FROM docker.io/paritytech/ci-linux:production as builder

LABEL maintainer="engineering@dhiway.com"
ARG PROFILE=production

WORKDIR /build
COPY . /build

RUN cargo build --locked --profile ${PROFILE}

# test
# RUN cargo test --release --all

# This is the 2nd stage: a very small image where we copy the Polkadot binary."
FROM gcr.io/distroless/cc-debian11@sha256:c2e1b5b0c64e3a44638e79246130d480ff09645d543d27e82ffd46a6e78a3ce3
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=production

COPY --from=builder /build/target/${PROFILE}/cord /cord

RUN ["/cord","--version"]

USER 1000:1000
EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/cord","-d","/data"]

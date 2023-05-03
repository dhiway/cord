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
FROM gcr.io/distroless/cc-debian11@sha256:3516ad5504f54aeaea0444dfd5044cc5969653e49b594cecb47dc400ea6f6820
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=production

COPY --from=builder /build/target/${PROFILE}/cord /cord

RUN ["/cord","--version"]

USER 1000:1000
EXPOSE 30333 9933 9944 9615
VOLUME ["/data"]

ENTRYPOINT ["/cord","-d","/data"]

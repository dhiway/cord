FROM docker.io/paritytech/ci-linux:production as builder

LABEL maintainer="engineering@dhiway.com"
ARG PROFILE=production

WORKDIR /build
COPY . /build

RUN cargo build --locked --profile ${PROFILE}

FROM gcr.io/distroless/cc-debian11@sha256:9b8e0854865dcaf49470b4ec305df45957020fbcf17b71eeb50ffd3bc5bf885d
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=production

COPY --from=builder /build/target/${PROFILE}/cord /cord

RUN ["/cord","--version"]

USER 1000:1000
EXPOSE 30333 9933 9944 9615

ENTRYPOINT ["/cord"]

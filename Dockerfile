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
FROM gcr.io/distroless/cc
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=production

COPY --from=builder /build/target/${PROFILE}/cord /usr/local/bin

RUN useradd -m -u 1000 -U -s /bin/sh -d /cord cord && \
	mkdir -p /data /cord/.local/share && \
	chown -R cord:cord /data && \
	ln -s /data /cord/.local/share/cord && \
	# unclutter and minimize the attack surface
	rm -rf /usr/bin /usr/sbin && \
	# check if executable works in this container
	/usr/local/bin/cord --version

USER cord

EXPOSE 30333 9933 9944 9615 
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/cord"]

# this container builds the cord binary from source files and the runtime library
# pinned the version to avoid build cache invalidation
# ===== FIRST (BUILD) STAGE ======

FROM docker.io/paritytech/ci-linux:production as builder

LABEL maintainer="engineering@dhiway.com"
ARG PROFILE=release

WORKDIR /build
COPY . /build

#build
RUN cargo build --locked --profile ${PROFILE}

# test
# RUN cargo test --release --all

# ===== SECOND STAGE ======

FROM docker.io/library/ubuntu:20.04
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=release

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

# checks
RUN /usr/local/bin/cord --version

EXPOSE 30333 9933 9944 9615 
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/cord"]

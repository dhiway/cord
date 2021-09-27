# this container builds the cord binary from source files and the runtime library
# pinned the version to avoid build cache invalidation
# ===== FIRST (BUILD) STAGE ======

FROM docker.io/paritytech/ci-linux:production as builder

LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=release

WORKDIR /build

COPY . /build

#build
RUN cargo build "--$PROFILE"

# test
# RUN cargo test --release --all

# ===== SECOND STAGE ======

FROM docker.io/library/ubuntu:20.04
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=release

# install tools and dependencies
RUN apt-get update && \
	DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
	libssl1.1 \
	ca-certificates \
	curl \
	gnupg && \
	useradd -m -u 1000 -U -s /bin/sh -d /cord cord && \
	apt-get autoremove -y && \
	apt-get clean -y && \
	rm -rf /var/lib/apt/lists/* ; \
	mkdir -p /data /cord/.local/share && \
	chown -R cord:cord /data && \
	ln -s /data /cord/.local/share/cord

COPY --from=builder /build/target/$PROFILE/cord /usr/local/bin

USER cord

# checks
RUN /usr/local/bin/cord --version

EXPOSE 30333 9933 9944 
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/cord"]

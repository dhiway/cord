# this container builds the cord binary from source files and the runtime library
# pinned the version to avoid build cache invalidation
# ===== FIRST (BUILD) STAGE ======

FROM paritytech/ci-linux:production as builder

LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=release

WORKDIR /build

COPY . /build

#build
RUN cargo build "--$PROFILE"

# test
# RUN cargo test --release --all

# ===== SECOND STAGE ======

FROM debian:bullseye-slim
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=release

# install tools and dependencies
RUN apt-get update && \
	DEBIAN_FRONTEND=noninteractive apt-get upgrade -y && \
	DEBIAN_FRONTEND=noninteractive apt-get install -y \
	libssl1.1 \
	ca-certificates \
	curl && \
	# apt cleanup
	apt-get autoremove -y && \
	apt-get clean -y && \
	find /var/lib/apt/lists/ -type f -not -name lock -delete; \
	# add user
	useradd -m -u 1000 -U -s /bin/sh -d /cord cord && \
	mkdir -p /cord/.local/share && \
	mkdir /data && \
	chown -R cord:cord /data && \
	ln -s /data /cord/.local/share/cord

COPY --from=builder /build/target/$PROFILE/cord /usr/local/bin

# checks
RUN ldd /usr/local/bin/cord && \
	/usr/local/bin/cord --version

# Shrinking
# RUN rm -rf /usr/lib/python* && \
# 	rm -rf /usr/bin /usr/sbin /usr/share/man

USER cord
EXPOSE 30333 9933 9944 
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/cord"]

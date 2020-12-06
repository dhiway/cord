# this container builds the cord-node binary from source files and the runtime library
# ===== FIRST (BUILD) STAGE ======

FROM debian:buster-slim as builder

LABEL maintainer="engineering@dhiway.com"

ENV DEBIAN_FRONTEND=noninteractive

ARG PROFILE=release

WORKDIR /build

COPY . /build

RUN apt-get update && \
	apt-get dist-upgrade -y -o Dpkg::Options::="--force-confold" && \
	apt-get install -y cmake pkg-config libssl-dev git clang curl
    
#build
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y && \
	export PATH="$PATH:$HOME/.cargo/bin" && \
	rustup toolchain install nightly-2020-10-06 && \
	rustup target add wasm32-unknown-unknown --toolchain nightly-2020-10-06 && \
    rustup override set nightly-2020-10-06 --path $WORKDIR/.. && \
	rustup default stable && \
	cargo build "--$PROFILE"

# test
#RUN cargo test --release --all

# ===== SECOND STAGE ======

FROM debian:buster-slim
LABEL maintainer="engineering@dhiway.com"

ARG PROFILE=release

# show backtraces
ENV RUST_BACKTRACE 1

# install tools and dependencies
RUN apt-get update && \
	DEBIAN_FRONTEND=noninteractive apt-get upgrade -y && \
	DEBIAN_FRONTEND=noninteractive apt-get install -y \
		libssl1.1 \
		ca-certificates \
		curl && \
# apt cleanup
	apt-get autoremove -y && \
	apt-get clean && \
	find /var/lib/apt/lists/ -type f -not -name lock -delete; \
# add user
	useradd -m -u 1000 -U -s /bin/sh -d /cord cord 

COPY --from=builder /build/target/$PROFILE/cord-node /usr/local/bin

# checks
RUN ldd /usr/local/bin/cord-node && \
	/usr/local/bin/cord-node --version

# Shrinking
RUN rm -rf /usr/lib/python* && \
	rm -rf /usr/bin /usr/sbin /usr/share/man

USER cord

EXPOSE 30333 9933 9944 
VOLUME ["/data"]

CMD ["/usr/local/bin/cord-node"]

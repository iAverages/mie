ARG DEBIAN_VERSION="12.1"

FROM --platform=$BUILDPLATFORM rust:1.72.1-slim-bookworm AS build

WORKDIR /app 

# Install target compilation targets
RUN dpkg --add-architecture arm64

RUN apt-get update && \
    apt-get install --yes \
    git build-essential crossbuild-essential-arm64 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

RUN rustup target add aarch64-unknown-linux-gnu
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/aarch64-linux-gnu-gcc

# Allow git (cargo) to clone b2 client repo
COPY ./docker/credentials /root/.git-credentials
RUN chmod 0600 ~/.git-credentials
RUN echo "[credential]\nhelper = store" > /root/.gitconfig
RUN git config --global credential.helper 'store --file /root/.git-credentials'

COPY Cargo.toml Cargo.lock ./

COPY src ./src

RUN CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release --target aarch64-unknown-linux-gnu


FROM --platform=$TARGETPLATFORM alpine:3.18.3 AS ytdlp

RUN apk update && apk add wget \
    && wget https://github.com/yt-dlp/yt-dlp/releases/download/2023.07.06/yt-dlp_linux_aarch64 -O /usr/local/bin/yt-dlp \
    && chmod +x /usr/local/bin/yt-dlp


FROM debian:${DEBIAN_VERSION}-slim

WORKDIR /app

RUN echo "deb http://deb.debian.org/debian bullseye-backports main" >> /etc/apt/sources.list
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install --yes \
    -t bullseye-backports python3 ca-certificates libpython3.8 \
    && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=build /app/target/aarch64-unknown-linux-gnu/release/mie /app/mie
COPY --from=ytdlp  /usr/local/bin/yt-dlp /usr/local/bin/yt-dlp

ENV PATH="/usr/local/bin:/app:${PATH}"

ENTRYPOINT  ["/app/mie"]
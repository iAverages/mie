ARG DEBIAN_VERSION="12.1"

FROM --platform=$BUILDPLATFORM rust:1.79.0-slim-bookworm AS build

WORKDIR /app 

# Install target compilation targets
RUN dpkg --add-architecture arm64

RUN apt-get update && \
    apt-get install --yes \
    git build-essential crossbuild-essential-arm64 pkg-config libssl-dev \
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

COPY .env .

COPY src ./src

RUN CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release --target aarch64-unknown-linux-gnu


FROM --platform=$TARGETPLATFORM alpine:3.18.4 AS ytdlp

RUN apk update && apk add wget libressl-dev pkgconfig \
    && wget https://github.com/yt-dlp/yt-dlp/releases/download/2023.11.16/yt-dlp_linux_aarch64 -O /usr/local/bin/yt-dlp \
    && chmod +x /usr/local/bin/yt-dlp \
    && wget https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-n7.0-latest-linuxarm64-gpl-7.0.tar.xz -O /tmp/ffmpeg.tar.xz \
    && tar -xf /tmp/ffmpeg.tar.xz -C /tmp --strip-components=1 \
    && rm -rf /tmp/ffmpeg.tar.xz

FROM debian:${DEBIAN_VERSION}-slim

WORKDIR /app

RUN echo "deb http://deb.debian.org/debian bullseye-backports main" >> /etc/apt/sources.list
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install --yes \
    -t bullseye-backports python3 ca-certificates libpython3.8 \
    && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

COPY --from=build /app/target/aarch64-unknown-linux-gnu/release/mie /app/mie
COPY --from=ytdlp  /usr/local/bin/yt-dlp /usr/local/bin/yt-dlp
COPY --from=ytdlp  /tmp/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=ytdlp  /tmp/bin/ffprobe /usr/local/bin/ffprobe

ENV PATH="/usr/local/bin:/app:${PATH}"

ENTRYPOINT  ["/app/mie"]
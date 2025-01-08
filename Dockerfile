FROM --platform=$BUILDPLATFORM rust:1.83.0 AS rust
ARG TARGETPLATFORM

RUN rustup target add aarch64-unknown-linux-gnu
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/aarch64-linux-gnu-gcc
RUN apt-get update && apt-get -y install build-essential crossbuild-essential-arm64 pkg-config libssl-dev gcc-arm-linux-gnueabihf gcc-aarch64-linux-gnu

WORKDIR /app
COPY .cargo ./.cargo
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=ssh --mount=type=cache,target=/app/target cargo build --release --target aarch64-unknown-linux-gnu
RUN --mount=type=cache,target=/app/target cp target/aarch64-unknown-linux-gnu/release/mie .
RUN --mount=type=cache,target=/app/target file mie

FROM debian:12.1-slim
ENV RUST_BACKTRACE=full
WORKDIR /app

RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install --yes \
    ca-certificates ffmpeg curl python3
RUN mkdir -p ~/.local/bin \
    && curl -L https://github.com/yt-dlp/yt-dlp/releases/download/2024.11.18/yt-dlp -o /root/.local/bin/yt-dlp \
    && chmod a+rx /root/.local/bin/yt-dlp \
    && DEBIAN_FRONTEND=noninteractive apt-get remove --yes curl && apt-get clean && rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENV YT_DLP="/root/.local/bin"
ENV PATH="$YT_DLP:$PATH"

COPY --from=rust /app/mie ./

CMD ["/app/mie"]


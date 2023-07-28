FROM --platform=$BUILDPLATFORM golang:1.20 AS build

WORKDIR /app

RUN apt update && apt install wget xz-utils -y \
    && wget https://github.com/yt-dlp/yt-dlp/releases/download/2023.07.06/yt-dlp_linux_aarch64 -O /usr/local/bin/yt-dlp \
    && chmod +x /usr/local/bin/yt-dlp \
    && mkdir -p /temp/ffmpeg \
    && wget https://github.com/yt-dlp/FFmpeg-Builds/releases/download/autobuild-2023-07-28-14-11/ffmpeg-N-111639-gb300552dfb-linuxarm64-gpl.tar.xz -O /tmp/ffmpeg.tar.xz \
    && tar -C /temp/ffmpeg -xvf /tmp/ffmpeg.tar.xz --wildcards "*/bin" \
    && cp /temp/ffmpeg/*/bin/ffmpeg /usr/local/bin/ffmpeg \
    && cp /temp/ffmpeg/*/bin/ffprobe /usr/local/bin/ffprobe \
    && ls -la /usr/local/bin \
    && rm /tmp/ffmpeg.tar.xz

## Copy go.mod and go.sum files, download dependencies so they are cached
COPY go.mod go.sum ./
RUN go mod download

# Copy sources
COPY cmd ./cmd
COPY internal ./internal

# Use for cross compilation 
ARG TARGETOS
ARG TARGETARCH

RUN CGO_ENABLED=0 \
    GOOS=$TARGETOS \
    GOARCH=$TARGETARCH \
     go build -ldflags="-s -w" -o mie ./cmd


# FROM --platform=$TARGETPLATFORM  alpine:3.18.2
# FROM --platform=$TARGETPLATFORM linuxserver/ffmpeg
FROM --platform=$TARGETPLATFORM centos

WORKDIR /app

COPY --from=build /app/mie /app/mie
COPY --from=build /usr/local/bin/yt-dlp /usr/local/bin/yt-dlp
COPY --from=build /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=build /usr/local/bin/ffprobe /usr/local/bin/ffprobe

ENV YTDL_PATH=/usr/local/bin/yt-dlp 
ENV FFMPEG_PATH=/usr/local/bin/ffmpeg 

# app.env file needs to exist otherwise viper will complain
COPY app.env.def /app/app.env

ENTRYPOINT  ["/app/mie"]
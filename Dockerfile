FROM --platform=$TARGETPLATFORM python:alpine3.18 AS ytdlp_builder

WORKDIR /temp/yt-dlp

RUN apk update && apk add git binutils \
    && git clone https://github.com/yt-dlp/yt-dlp.git \
    && cd yt-dlp \
    && python3 -m pip install -U pyinstaller -r requirements.txt \
    && python3 devscripts/make_lazy_extractors.py \
    && python3 pyinst.py 

FROM --platform=$BUILDPLATFORM golang:1.21@sha256:c416ceeec1cdf037b80baef1ccb402c230ab83a9134b34c0902c542eb4539c82 AS build

WORKDIR /app

RUN apt update && apt install wget xz-utils -y \
    && wget https://github.com/yt-dlp/yt-dlp/releases/download/2023.07.06/yt-dlp_linux_aarch64 -O /usr/local/bin/yt-dlp \
    && chmod +x /usr/local/bin/yt-dlp \
    && mkdir -p /temp/ffmpeg \
    && wget https://github.com/yt-dlp/FFmpeg-Builds/releases/download/autobuild-2023-09-01-16-23/ffmpeg-N-111873-gfba4546175-linuxarm64-gpl.tar.xz -O /tmp/ffmpeg.tar.xz \
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


FROM --platform=$TARGETPLATFORM  alpine:3.18.3@sha256:7144f7bab3d4c2648d7e59409f15ec52a18006a128c733fcff20d3a4a54ba44a

WORKDIR /app

COPY --from=build /app/mie /app/mie
COPY --from=ytdlp_builder /temp/yt-dlp/yt-dlp/dist/yt-dlp_linux_aarch64 /usr/local/bin/yt-dlp
COPY --from=build /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=build /usr/local/bin/ffprobe /usr/local/bin/ffprobe

ENV YTDL_PATH=/usr/local/bin/yt-dlp 
ENV FFMPEG_PATH=/usr/local/bin/ffmpeg 

# app.env file needs to exist otherwise viper will complain
COPY app.env.def /app/app.env

ENTRYPOINT  ["/app/mie"]
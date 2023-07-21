FROM --platform=$BUILDPLATFORM golang:1.20 AS build

WORKDIR /app

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

RUN apt update && apt install wget \
    && wget https://github.com/yt-dlp/yt-dlp/releases/download/2023.07.06/yt-dlp_linux_aarch64 -O /usr/local/bin/yt-dlp \
    && chmod +x /usr/local/bin/yt-dlp

# FROM --platform=$TARGETPLATFORM  alpine:3.18.2
FROM --platform=$TARGETPLATFORM centos

WORKDIR /app

COPY --from=build /app/mie /app/mie
COPY --from=build /usr/local/bin/yt-dlp /usr/local/bin/yt-dlp

ENV YTDL_PATH=/usr/local/bin/yt-dlp 

# app.env file needs to exist otherwise viper will complain
COPY app.env.def /app/app.env

CMD ["/app/mie"]
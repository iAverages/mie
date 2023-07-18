FROM --platform=$BUILDPLATFORM golang:1.20 AS build

WORKDIR /app

## Copy go.mod and go.sum files, download dependencies so they are cached
COPY go.mod go.sum ./
RUN go mod download

# Copy sources
COPY cmd ./cmd
COPY internal ./internal

ARG TARGETOS
ARG TARGETARCH

# RUN wget -O /usr/local/bin/yt-dlp https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_$(uname -m | sed 's/_64//')
# RUN chmod +x /usr/local/bin/yt-dlp

RUN CGO_ENABLED=0 \
    GOOS=$TARGETOS \
    GOARCH=$TARGETARCH \
     go build -ldflags="-s -w" -o mie ./cmd

FROM --platform=$BUILDPLATFORM alpine:3.14
WORKDIR /app

COPY --from=build /app/mie /app/mie

# app.env file needs to exist otherwise viper will complain
COPY app.env.def /app/app.env

EXPOSE 8000

CMD ["/app/mie"]
FROM golang:1.20 AS build

WORKDIR /app

## Copy go.mod and go.sum files, download dependencies so they are cached
COPY go.mod go.sum ./
RUN go mod download

# Copy sources
COPY cmd ./cmd
COPY internal ./internal

RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o mie ./cmd

FROM alpine:3.18.2 AS app

WORKDIR /app

COPY --from=build /app/mie /app/mie

# app.env file needs to exist otherwise viper will complain
COPY app.env.def /app/app.env

EXPOSE 8000

CMD ["/app/mie"]
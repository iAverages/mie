BINARY_NAME = user-cache

build:
	go build -o $(BINARY_NAME) -v ./cmd

docker:
	docker buildx  build --platform=linux/arm64 -t ctr.avrg.dev/mie:dev --load .

docker-push:
	docker buildx  build --platform=linux/arm64 -t ctr.avrg.dev/mie:dev --push .
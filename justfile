BINARY_NAME := "mie"
DOCKER_IMAGE_NAME := "ctr.avrg.dev/" + BINARY_NAME 
HASH := `git rev-parse --short HEAD`

docker:
	docker buildx build --platform=linux/arm64 -t {{ DOCKER_IMAGE_NAME }}:{{ HASH }} --push . 

docker_latest:
	docker buildx build --platform=linux/arm64 -t {{ DOCKER_IMAGE_NAME }}:latest --push . 


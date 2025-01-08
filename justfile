BINARY_NAME := "mie"
DOCKER_IMAGE_NAME := "ctr.dan.local/" + BINARY_NAME 
HASH := `git rev-parse --short HEAD`

docker:
	docker buildx build --ssh default --platform=linux/arm64 -t {{ DOCKER_IMAGE_NAME }}:{{ HASH }} --load . 

docker_latest:
    docker buildx build --ssh default --platform=linux/arm64 -t {{ DOCKER_IMAGE_NAME }}:latest --load .

docker_push:
    docker push {{ DOCKER_IMAGE_NAME }}:{{ HASH }}

docker_latest_push:
    docker push {{ DOCKER_IMAGE_NAME }}:latest

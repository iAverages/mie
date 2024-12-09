BINARY_NAME := "mie"
DOCKER_IMAGE_NAME := "ctr.dan.local/" + BINARY_NAME 
HASH := `git rev-parse --short HEAD`

docker:
	docker buildx build --ssh default  --platform=linux/arm/v7 -t {{ DOCKER_IMAGE_NAME }}:{{ HASH }} --push . 

docker_latest:
	docker buildx build --ssh default --platform=linux/arm/v7 -t {{ DOCKER_IMAGE_NAME }}:latest --load . 

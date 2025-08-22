BINARY_NAME := "mie"
DOCKER_IMAGE_NAME := "ctr.dan.local/" + BINARY_NAME 
HASH := `git rev-parse --short HEAD`

docker:
    nix build .#dockerImage
    docker load < result

docker_latest_push:
    docker push {{ DOCKER_IMAGE_NAME }}:latest

docker:
	docker buildx build --platform=linux/arm64 -t pihole.dan.local:5000/mie:dev --push . 

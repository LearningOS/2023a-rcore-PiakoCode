DOCKER_NAME ?= rcore-tutorial-v3
.PHONY: docker build_docker
	
CONTAINER_NAME = rCore

docker:
	docker run --rm -it -v ${PWD}:/mnt -w /mnt ${DOCKER_NAME} bash

build_docker: 
	docker build -t ${DOCKER_NAME} .

exec_docker:
	docker restart ${CONTAINER_NAME} && docker exec -it ${CONTAINER_NAME} /bin/bash -c "cd /mnt/os; bash";
# docker stop ${CONTAINER_NAME}

fmt:
	cd os ; cargo fmt;  cd ..


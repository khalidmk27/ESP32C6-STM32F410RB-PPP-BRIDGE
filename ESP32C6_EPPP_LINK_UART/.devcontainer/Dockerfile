# ARG DOCKER_TAG=latest
# FROM espressif/idf:${DOCKER_TAG}
FROM espressif/idf:release-v5.4@sha256:151694440c974ab29fad02137dc0fbb4b6c4f9a2586d7c7f1e8ec8288386186d
ENV LC_ALL=C.UTF-8
ENV LANG=C.UTF-8

RUN apt-get update -y && apt-get install udev -y

RUN echo "source /opt/esp/idf/export.sh > /dev/null 2>&1" >> ~/.bashrc

ENTRYPOINT [ "/opt/esp/entrypoint.sh" ]

CMD ["/bin/bash", "-c"]
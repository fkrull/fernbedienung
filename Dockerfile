FROM docker.io/library/alpine:3.17.2
RUN apk add --no-cache mpc
COPY fernbedienung /usr/local/bin/fernbedienung
RUN chmod 0755 /usr/local/bin/fernbedienung
CMD ["/usr/local/bin/fernbedienung"]

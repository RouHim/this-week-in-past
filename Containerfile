# # # # # # # # # # # # # # # # # # # #
# Builder
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM docker.io/alpine:3 as builder

# Create an empty directory that will be used in the final image
RUN mkdir "/empty_dir"

# Install ssl certificates that will also be copied into the final image
RUN apk update && apk add --no-cache \
    ca-certificates bash file

# Copy all archs in to this container
RUN mkdir /bin
WORKDIR /bin
COPY target .
COPY stage-arch-bin.sh /bin

# This will copy the cpu arch corresponding binary to /target/this-week-in-past
RUN bash stage-arch-bin.sh this-week-in-past

# # # # # # # # # # # # # # # # # # # #
# Run image
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM scratch

ENV USER "1337"
ENV RESOURCE_PATHS "/resources"
ENV DATA_FOLDER "/data"
ENV RUST_LOG "info"

# For performance reasons write data to docker volume instead of containers writeable fs layer
VOLUME $DATA_FOLDER

# Copy the empty directory as data and temp folder
COPY --chown=$USER:$USER --from=builder /empty_dir $DATA_FOLDER
COPY --chown=$USER:$USER --from=builder /empty_dir /tmp

# Copy ssl certificates to the scratch image to enable HTTPS requests
COPY --chown=$USER:$USER --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the built application from the build image to the run-image
COPY --chown=$USER:$USER --from=builder /target/this-week-in-past /this-week-in-past

# Copy the static html website data from the host to the container
COPY --chown=$USER:$USER web-app /web-app

EXPOSE 8080
USER $USER

CMD ["/this-week-in-past"]

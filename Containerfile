# # # # # # # # # # # # # # # # # # # #
# Builder
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM docker.io/alpine:3 as builder

# Create an empty directory that will be used in the final image
RUN mkdir "/empty_dir"

# Install alpine-sdk that provides build dependencies
# Install ssl certificates that will also be copied into the final image
# Install pavao (smb client) required dependencies
RUN apk update && apk add --no-cache \
    ca-certificates

# # # # # # # # # # # # # # # # # # # #
# Run image
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM scratch

ARG BINARY_FILE

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

# Copy the built application from the host to the container
COPY --chown=$USER:$USER $BINARY_FILE /this-week-in-past

# Copy the static html website data from the host to the container
COPY --chown=$USER:$USER web-app /web-app

EXPOSE 8080
USER $USER

CMD ["/this-week-in-past"]

# # # # # # # # # # # # # # # # # # # #
# Base image
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM alpine as base

# Create a cache directory that will be copied into the final image
RUN mkdir "/cache"

# Install ssl certificates that will also be copied into the final image
RUN apk update && apk add --no-cache ca-certificates

# # # # # # # # # # # # # # # # # # # #
# Run image
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM scratch as runtime

ENV CACHE_DIR "/cache"
ENV RESOURCE_PATHS "/resources"
ARG TARGET_PLATFORM

VOLUME /cache

# Create an empty cache directory
COPY --chown=1337:1337 --from=base /cache /cache

# Copy ssl certificates to the scratch image to enable HTTPS
COPY --chown=1337:1337 --from=base /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the built application from the host to the container
COPY --chown=1337:1337 ./target/$TARGET_PLATFORM/release/this-week-in-past /this-week-in-past

# Copy the static html website data from the host to the container
COPY --chown=1337:1337 ./static /static

EXPOSE 8080
USER 1337

CMD ["/this-week-in-past"]
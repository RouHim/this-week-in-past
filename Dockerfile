FROM alpine AS base
RUN mkdir "/cache"
RUN apk update && apk add --no-cache ca-certificates

FROM scratch AS runtime
ENV CACHE_DIR "/cache"
ENV RESOURCE_PATHS "/resources"

VOLUME /cache

# Create an empty cache directory
COPY --chown=1337:1337 --from=base /cache /cache

# Copy ssl certificates to the scratch image to enable HTTPS
COPY --chown=1337:1337 --from=base /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Copy the built application from the host to the container
COPY --chown=1337:1337 ./target/x86_64-unknown-linux-musl/release/this-week-in-past /this-week-in-past

# Copy the static html website data from the host to the container
COPY --chown=1337:1337 ./static /static

EXPOSE 8080
USER 1337

CMD ["/this-week-in-past"]
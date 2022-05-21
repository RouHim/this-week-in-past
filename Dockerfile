FROM alpine AS base
RUN mkdir "/cache"
FROM scratch AS runtime
ENV CACHE_DIR "/cache"
ENV RESOURCE_PATHS "/resources"

VOLUME /cache

COPY --chown=1337:1337 --from=base /cache /cache
COPY --chown=1337:1337 ./target/x86_64-unknown-linux-musl/release/this-week-in-past /this-week-in-past
COPY --chown=1337:1337 ./static /static

EXPOSE 8080
USER 1337

CMD ["/this-week-in-past"]
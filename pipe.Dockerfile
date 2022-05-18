# The actual image to run
FROM scratch AS runtime
ENV RESOURCE_PATHS=/resources
COPY ./target/x86_64-unknown-linux-musl/release/this-week-in-past /app
COPY ./static /static
VOLUME /cache
EXPOSE 8080
USER 1337
CMD ["/app"]
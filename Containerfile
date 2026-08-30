# # # # # # # # # # # # # # # # # # # #
# GeoNames data — ~10 MB uncompressed (~185k cities), baked to /cities500.txt
# Final image +~10 MB; pinned alpine for reproducibility, single layer to minimize cache invalidation
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM docker.io/alpine:3.21 AS geodata
RUN apk add --no-cache curl unzip && \
    curl -fL --retry 3 --retry-delay 2 --connect-timeout 15 https://download.geonames.org/export/dump/cities500.zip -o /tmp/cities500.zip && \
    unzip -p /tmp/cities500.zip > /cities500.txt && \
    test -s /cities500.txt || { echo "cities500.txt is empty after unzip"; exit 1; } && \
    lines=$(wc -l < /cities500.txt) && test "$lines" -gt 100000 || { echo "cities500 too small: $lines lines"; exit 1; } && \
    test "$(stat -c%s /cities500.txt)" -gt 5000000 || { echo "cities500 too small by size"; exit 1; } && \
    wc -l /cities500.txt && rm /tmp/cities500.zip
# # # # # # # # # # # # # # # # # # # #
# Builder
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM docker.io/alpine AS builder

# Create an empty directory that will be used in the final image
RUN mkdir "/empty_dir"

# Install required packages for the staging script
RUN apk update && apk add --no-cache bash file

# Copy all archs into this container
RUN mkdir /work
WORKDIR /work
COPY target .
COPY .container/stage-arch-bin.sh /work

# This will copy the cpu arch corresponding binary to /target/this-week-in-past
RUN bash stage-arch-bin.sh this-week-in-past

# # # # # # # # # # # # # # # # # # # #
# Run image
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM scratch

ENV USER="1337"
ENV RESOURCE_PATHS="/resources"
ENV DATA_FOLDER="/data"
ENV RUST_LOG="info"

# For performance reasons write data to docker volume instead of containers writeable fs layer
VOLUME $DATA_FOLDER

# Copy the empty directory as data and temp folder
COPY --chown=$USER:$USER --from=builder /empty_dir $DATA_FOLDER
COPY --chown=$USER:$USER --from=builder /empty_dir /tmp

# Copy the built application from the build image to the run-image
COPY --chown=$USER:$USER --from=builder /work/this-week-in-past /this-week-in-past

# Copy offline city database
COPY --from=geodata /cities500.txt /cities500.txt
EXPOSE 8080
USER $USER

ENTRYPOINT ["/this-week-in-past"]

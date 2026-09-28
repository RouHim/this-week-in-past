# # # # # # # # # # # # # # # # # # # #
# GeoNames place data — cities500.zip plus the allCountries.zip PPLX (section/quarter) rows,
# merged and validated by .container/build-geodata.sh into ~17 MB raw (~396,100 rows) at /geodata.txt
# Header: "#twip-places-v1" + name/lat/lon/feature_class/feature_code/country_code/population
# Final image +~17 MB; pinned alpine for reproducibility, single layer to minimize cache invalidation
# Pinned to the build platform: the merged file is target-independent, so buildx generates it once
# for all target images instead of once per platform (three of them under QEMU).
# # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # # #
FROM --platform=$BUILDPLATFORM docker.io/alpine:3.24 AS geodata
RUN apk add --no-cache bash curl unzip
COPY .container/build-geodata.sh /build-geodata.sh
RUN bash /build-geodata.sh /geodata.txt
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

# Copy offline place dataset
COPY --from=geodata /geodata.txt /geodata.txt
EXPOSE 8080
USER $USER

ENTRYPOINT ["/this-week-in-past"]

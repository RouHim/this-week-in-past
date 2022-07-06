<p align="center">
  <img src="https://raw.githubusercontent.com/RouHim/this-week-in-past/main/banner.png" width="500">
</p>

<p align="center">
    <a href="https://github.com/RouHim/this-week-in-past/actions/workflows/build-image.yaml"><img src="https://github.com/RouHim/this-week-in-past/actions/workflows/build-image.yaml/badge.svg" alt="CI"></a>
    <a href="https://github.com/RouHim/this-week-in-past/actions/workflows/scheduled-security-audit.yaml"><img src="https://github.com/RouHim/this-week-in-past/actions/workflows/scheduled-security-audit.yaml/badge.svg" alt="CI"></a>
    <a href="https://hub.docker.com/r/rouhim/this-week-in-past"><img alt="Docker Pulls" src="https://img.shields.io/docker/pulls/rouhim/this-week-in-past"></a>
    <a href="https://hub.docker.com/r/rouhim/this-week-in-past/tags"><img alt="Docker Image Size (tag)" src="https://img.shields.io/docker/image-size/rouhim/this-week-in-past/latest"></a>
</p>

<p align="center">
    <i>Aggregate images taken this week, from previous years and presents them on a web page with slideshow.</i>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/RouHim/this-week-in-past/main/screenshot.jpg" width="500">
</p>

## Motivation

When I migrated my photo collection from google photos to a locally hosted instance of photoprism, I missed the
automatically generated slideshow feature of google photos, here it is now.

## Performance

Hardware: i3-12100T, 3xWD_BLACK SN750 (RAID-Z1), 32GB RAM

Photos: ~80k

The first indexing when starting the application took about 6 seconds.

## Run the application

The application should be started as a container.

Docker Example:

```shell
docker run -p 8080:8080 \
        -v /path/to/pictures:/resources \
        -e SLIDESHOW_INTERVAL=10 \
        -e WEATHER_ENABLED=true \
        -e OPEN_WEATHER_MAP_API_KEY=<YOUR_KEY> \
        -e BIGDATA_CLOUD_API_KEY=<YOUR_KEY> \
        rouhim/this-week-in-past
```

Docker compose example:
```shell
version: "3.9"

services:
  this-week-in-past:
    image: rouhim/this-week-in-past
    volumes:
      - ~/Pictures/:/resources:ro # should be read only
    ports:
      - "8080:8080"
```

## Configuration

All configuration is done via environment variables:

| Name                     | Description                                                                                           | Default value |
|--------------------------|-------------------------------------------------------------------------------------------------------|---------------|
| RESOURCE_PATHS           | Paths to the resources to load (comma separated), container defaults this to `/resources`             |               |
| CACHE_DIR                | Path to the caching to load, needs to read/write rights, must not be set when using container         |               |
| SLIDESHOW_INTERVAL       | Interval of the slideshow in seconds                                                                  | 30            |
| DATE_FORMAT              | Date format of the image taken date (https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html) | %d.%m.%Y      |
| BIGDATA_CLOUD_API_KEY    | To resolve geo coordinates to city name. Obtain here: https://www.bigdatacloud.com                    |               |
| OPEN_WEATHER_MAP_API_KEY | To receive weather live data. Obtain here: https://openweathermap.org/api                             |               |
| WEATHER_ENABLED          | Indicates if weather should be shown in the slideshow                                                 | false         |
| WEATHER_LOCATION         | Weather location                                                                                      | Berlin        |
| WEATHER_LANGUAGE         | Weather language                                                                                      | en            |
| WEATHER_UNIT             | Weather units (metric or imperial)                                                                    | metric        |
| HOME_ASSISTANT_BASE_URL  | Home assistant base url                                                                               |               |
| HOME_ASSISTANT_ENTITY_ID | Home assistant entity id to load the weather from                                                     |               |
| HOME_ASSISTANT_API_TOKEN | Home assistant api access token                                                                       |               |

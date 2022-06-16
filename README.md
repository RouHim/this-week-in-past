<p align="center">
  <img src="https://raw.githubusercontent.com/RouHim/this-week-in-past/main/banner.png" width="500">
</p>

<p align="center">
    <a href="https://github.com/RouHim/this-week-in-past/actions/workflows/build-image.yaml"><img src="https://github.com/RouHim/this-week-in-past/actions/workflows/build-image.yaml/badge.svg" alt="CI"></a>
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

Example:

```bash
docker run -p 8080:8080 \
        -v /path/to/pictures:/resources \
        rouhim/this-week-in-past
```

## Configuration

All configuration is done via environment parameter:

| Name                     | Description                                                                        | Default value |
|--------------------------|------------------------------------------------------------------------------------|---------------|
| RESOURCE_PATHS           | Paths to the resources to load (comma separated)                                   |               |
| CACHE_DIR                | Path to the caching to load, needs to read/write rights                            |               |
| SLIDESHOW_INTERVAL       | Interval of the slideshow in seconds                                               | 30            |
| WEATHER_ENABLED          | Indicates if weather should be shown in the slideshow                              | false         |
| BIGDATA_CLOUD_API_KEY    | To resolve geo coordinates to city name. Obtain here: https://www.bigdatacloud.com |               |
| OPEN_WEATHER_MAP_API_KEY | To receive weather live data. Obtain here: https://openweathermap.org/api          |               |
| LOCATION_NAME            | Weather location                                                                   | Berlin        |
| LANGUAGE                 | Weather language                                                                   | en            |
| HOME_ASSISTANT_BASE_URL  | Home assistant base url                                                            |               |
| HOME_ASSISTANT_ENTITY_ID | Home assistant entity id to load the weather from                                  |               |
| HOME_ASSISTANT_API_TOKEN | Home assistant api access token                                                    |               |

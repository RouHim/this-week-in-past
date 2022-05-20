# This week in past

Aggregates images taken this week, from previous years and presents them on a web page with slideshow.

## Run the application

The application should be started as a container.

Example:

```bash
docker run -p 8080:8080 
        -e RESOURCE_PATHS=/resources
        -e CACHE_DIR=/cache 
        -e WEATHER_ENABLED=false 
        -v /path/to/pictures:/resources 
        -v /path/to/cache:/cache # 
        rouhim/this-week-in-past
```

## Configuration

All configuration is done via environment parameter:

| Name                     | Description                                             | Default value |
|--------------------------|---------------------------------------------------------|---------------|
| RESOURCE_PATHS           | Paths to the resources to load (comma separated)        |               |
| CACHE_DIR                | Path to the caching to load, needs to read/write rights |               |
| SLIDESHOW_INTERVAL       | Interval of the slideshow in milliseconds               | 10000         |
| WEATHER_ENABLED          | Indicates if weather should be shown in the slideshow   | true          |
| LOCATION_NAME            | Weather location                                        | Berlin        |
| LANGUAGE                 | Weather language                                        | en            |
| HOME_ASSISTANT_BASE_URL  | Home assistant base url                                 |               |
| HOME_ASSISTANT_ENTITY_ID | Home assistant entity id to load the weather from       |               |
| HOME_ASSISTANT_API_TOKEN | Home assistant api access token                         |               |

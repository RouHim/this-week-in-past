# This week in past

Aggregates images taken this week, from previous years and presents them on a web page with slideshow.

## Run the application

The application can only be started as container.

Example:

```bash
docker run -p 8080:8080 -v /path/to/pictures:/resources rouhim/this-week-in-past
```

## Configuration

All configuration is done via environment parameter:

| Name      | Description                                                         | Default value |
|-----------|---------------------------------------------------------------------|---------------|
| RESOURCE_PATHS    | Path to the resources to load                                       | /resources    |
| WEATHER_ENABLED    | Indicates if weather should be shown in the sllideshow              | true          |
| LOCATION_NAME | Weather location                                                    | Berlin        |
| LANGUAGE | Weather languange                                                   | en            |
| HOME_ASSISTANT_BASE_URL | Home assistant base url                                             |             |
| HOME_ASSISTANT_ENTITY_ID | Home assistant entity id to load the weather from                   |             |
| HOME_ASSISTANT_API_TOKEN | Home assistant api access token                                     |             |

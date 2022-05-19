let resources;
let currentIndex = 0;
let maxIndex = 0;

// Initialize the slideshow
window.onload = () => {
    loadAvailableImages();
    loadWeatherInformation();

    // Reload page every hour
    setInterval(() => location.reload(), 3600000);
};

/// Checks if the weather information should be shown, if so load them
function loadWeatherInformation() {
    fetch(`${window.location.href}api/weather`)
        .then(response => response.json())
        .then(showWeather => {
            if (showWeather === true) {
                loadCurrentWeather();
            }
        });
}

// Shows the current weather on the slideshow
function loadCurrentWeather() {
    fetch(`${window.location.href}api/weather/current`)
        .then(response => response.json())
        .then(data => {
            showCurrentWeather(data);
        });
}

// Shows the current weather on the slideshow
function showCurrentWeather(data) {
    const weather = data.weather[0];
    const icon = weather.icon;

    document.getElementById("weather-label").innerHTML = weather.description + ",&nbsp;";
    document.getElementById("weather-icon").src = `https://openweathermap.org/img/w/${icon}.png`;

    if (isHomeAssistantEnabled()) {
        let homeAssistantData = JSON.parse(getCurrentTemperatureDataFromHomeAssistant());
        document.getElementById("weather-temperature").innerText =
            Math.round(homeAssistantData.state) + homeAssistantData.attributes.unit_of_measurement;
    } else {
        document.getElementById("weather-temperature").innerText =
            Math.round(data.main.temp) + "°C";
    }
}

// Returns true if Home Assistant is enabled
function isHomeAssistantEnabled() {
    let request = new XMLHttpRequest();
    request.open('GET', `${window.location.href}api/weather/homeassistant`, false);
    request.send(null);
    if (request.status === 200) {
        return String(request.responseText) === "true";
    }

    return false;
}

// Loads the current temperature from Home Assistant
function getCurrentTemperatureDataFromHomeAssistant() {
    let request = new XMLHttpRequest();
    request.open('GET', `${window.location.href}api/weather/homeassistant/temperature`, false);
    request.send(null);
    if (request.status === 200) {
        return request.response;
    }
}


// Set the slideshow image and its meta information on tick interval
function slideshowTick() {
    // set image
    let photoDataRequest = new XMLHttpRequest();
    photoDataRequest.open("GET",
        `${window.location.href}api/resources/${resources[currentIndex]}/${window.screen.availWidth}/${window.screen.availHeight}/base64`
    );
    photoDataRequest.send();
    photoDataRequest.onload = () => document.getElementById("slideshow-image").src = photoDataRequest.response;

    // set image description
    let photoMetadataRequest = new XMLHttpRequest();
    photoMetadataRequest.open("GET", window.location.href + "api/resources/" + resources[currentIndex] + "/description");
    photoMetadataRequest.send();
    photoMetadataRequest.onload = () => document.getElementById("slideshow-metadata").innerText = photoMetadataRequest.response;

    currentIndex++;
    if (currentIndex > maxIndex) {
        currentIndex = 0;
    }
}

// Starts the slideshow
function startSlideshow(response) {
    resources = response;
    maxIndex = Object.keys(resources).length - 1;
    slideshowTick();

    // Slideshow tick every 10 seconds
    setInterval(() => slideshowTick(), 10000);
}

// Loads the available images from the server
function loadAvailableImages() {
    // load all images of this week in the past years
    const http = new XMLHttpRequest();
    http.open("GET", window.location.href + "api/resources/week");
    http.send();
    http.responseType = "json"
    http.onload = () => startSlideshow(http.response);
}
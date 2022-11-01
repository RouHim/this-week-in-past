/*
Disclaimer:
    Yes this is vanilla javascript, and no I'm not a professional web developer.
*/

let resourcesThisWeek;
let currentIndex = 0;
let maxIndex = 0;
let current_resource_id;
let intervalID;

// On page load, do the following things:
// 1. Load the available images and initialize the slideshow with it
// 2. Load and show the weather information

// 3. Set a page reload interval for each hour
window.onload = () => {
    loadAvailableImages();
    loadWeatherInformation();
    initHideButton();

    // Reload page every x minutes
    let refreshIntervalInMinutes = getRefreshInterval();
    intervalID = setInterval(() => location.reload(), refreshIntervalInMinutes * 60000);
};

function initHideButton() {
    fetch(`${window.location.href}api/config/show-hide-button`)
        .then(response => response.json())
        .then(showHideButton => {
            if (showHideButton === true) {
                let hideCurrentImageBtn = document.getElementById("hide-current-image");
                hideCurrentImageBtn.style.visibility = "visible";
                hideCurrentImageBtn.addEventListener("click", hideCurrentImage)
            }
        });
}

function hideCurrentImage() {
    let hideResourceRequest = new XMLHttpRequest();
    hideResourceRequest.open("POST", window.location.href + "api/resources/hide/" + current_resource_id);
    hideResourceRequest.send();

    // Reload page if the resource is set to hidden
    hideResourceRequest.onload = () => location.reload();
}

// Checks if the weather information should be shown, if so load them
function loadWeatherInformation() {
    fetch(`${window.location.href}api/weather`)
        .then(response => response.json())
        .then(showWeather => {
            if (showWeather === true) {
                loadCurrentWeather();
            }
        });
}

// Loads the current weather from the rest api and shows it
function loadCurrentWeather() {
    fetch(`${window.location.href}api/weather/current`)
        .then(response => response.json())
        .then(data => {
            showCurrentWeather(data);
        });
}

// Shows the actual weather on the frontend
// If home assistant is enabled, the temperature is loaded from Home Assistant
// The weather icon is loaded from OpenWeatherMap
// @param data: the weather data
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

// Sets the image url and its meta information to the frontend
// This is done by fading out the current image and fading in the new image
// The sleep function is used to prevent the slideshow from flickering
// @param resource_id: the id of the resource
function setImage(resource_id) {
    current_resource_id = resource_id;

    // build the image url
    let imageUrl = `${window.location.href}api/resources/${resource_id}/${window.screen.availWidth}/${window.screen.availHeight}`;

    // obtain the image elements
    let backgroundImage = document.getElementById('background-image');
    let slideshowImage = document.getElementById("slideshow-image");
    let slideShowMetadata = document.getElementById("slideshow-metadata");

    // start the fade out animation
    backgroundImage.classList.add("fade-out");
    slideshowImage.classList.add("fade-out");
    slideShowMetadata.classList.add("fade-out");

    // wait for the fade out animation to end
    sleep(1000).then(() => {

        // when the image is loaded, start the fade in animation
        slideshowImage.onload = () => {
            // fade images in
            backgroundImage.classList.add("fade-in");
            backgroundImage.classList.remove("fade-out");

            slideshowImage.classList.add("fade-in");
            slideshowImage.classList.remove("fade-out");

            slideShowMetadata.classList.add("fade-in");
            slideShowMetadata.classList.remove("fade-out");

            // wait for the fade in animation to end
            sleep(1000).then(() => {
                backgroundImage.classList.remove("fade-in");
                slideshowImage.classList.remove("fade-in");
                slideShowMetadata.classList.remove("fade-in");
            });
        }

        // set image and blurred background image
        backgroundImage.style.backgroundImage = `url(${imageUrl})`;
        slideshowImage.src = imageUrl;

        // set image description but fade in is done simultaneously with the fade in of the image, see above
        let photoMetadataRequest = new XMLHttpRequest();
        photoMetadataRequest.open("GET", window.location.href + "api/resources/" + resource_id + "/description");
        photoMetadataRequest.send();
        photoMetadataRequest.onload = () => slideShowMetadata.innerText = photoMetadataRequest.response;
    })
}

// Returns a random resource from the backend API
function getRandomResource() {
    let request = new XMLHttpRequest();
    request.open('GET', `${window.location.href}api/resources/random`, false);
    request.send(null);

    if (request.status === 200) {
        return JSON.parse(request.response);
    } else {
        return null;
    }
}

// On slideshow tick interval
// Set the slideshow image and its meta information
function slideshowTick() {
    // If no image for this week was found, select a random one
    // If no random could be found, show an alert message and stop slideshow
    if (resourcesThisWeek.length === 0) {
        let randomResource = getRandomResource();

        if (randomResource === null) {
            alert("Could not find any photos!");
            clearInterval(intervalID);
            return;
        }

        setImage(randomResource);
        return;
    }

    // Proceeds with the regular "this week" slideshow
    setImage(resourcesThisWeek[currentIndex]);

    currentIndex++;
    if (currentIndex > maxIndex) {
        currentIndex = 0;
    }
}

// Loads the available images of this week from the backend API
// The response is used to start the slideshow
function loadAvailableImages() {
    // load all images of this week in the past years
    const http = new XMLHttpRequest();
    http.open("GET", window.location.href + "api/resources/week");
    http.send();
    http.responseType = "json"
    http.onload = () => startSlideshow(http.response);
}

// Returns the slideshow interval in seconds from the backend API
function getSlideshowInterval() {
    let request = new XMLHttpRequest();
    request.open('GET', `${window.location.href}api/config/interval/slideshow`, false);
    request.send(null);
    if (request.status === 200) {
        return request.responseText;
    }
    return 30;
}

// Returns the refresh interval in minutes from the backend API
function getRefreshInterval() {
    let request = new XMLHttpRequest();
    request.open('GET', `${window.location.href}api/config/interval/refresh`, false);
    request.send(null);
    if (request.status === 200) {
        return request.responseText;
    }
    return 180;
}

// Starts the slideshow utilizing `setInterval`
// The interval is set to the value returned from the backend API
function startSlideshow(foundResourcesOfThisWeek) {
    resourcesThisWeek = foundResourcesOfThisWeek;

    maxIndex = Object.keys(resourcesThisWeek).length - 1;
    slideshowTick();

    // Load slideshow interval
    let intervalInSeconds = getSlideshowInterval();

    // Start image slideshow
    setInterval(() => slideshowTick(), intervalInSeconds * 1000);
}

// Sleeps for the given amount of milliseconds and returns a promise
// that is resolved when the sleep is finished
// @param ms: the amount of milliseconds to sleep
function sleep(ms) {
    return new Promise(resolver => setTimeout(resolver, ms));
}
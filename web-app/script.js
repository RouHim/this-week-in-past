/*
Disclaimer:
    Yes this is vanilla javascript, and no I'm not a professional web developer.
*/

let resourcesThisWeek;
let currentIndex = 0;
let maxIndex = 0;
let current_resource_id;
let intervalID;
let forceRandomSlideshow = false;

/**
 * On page load, do the following things:
 *      - Load the available images and initialize the slideshow with it
 *      - Load and show the weather information
 *      - Set a page reload interval for each hour
 */
window.addEventListener('load', () => {
    initSlideshow();
    loadWeatherInformation();
    initHideButton();
    forceRandomSlideshow = shouldOnlyPlayRandom();

    // Reload page every x minutes
    let refreshIntervalInMinutes = getRefreshInterval();
    intervalID = setInterval(() => location.reload(), refreshIntervalInMinutes * 60000);
});

function shouldOnlyPlayRandom() {
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.has('RANDOM_SLIDESHOW')) {
        return urlParams.get('RANDOM_SLIDESHOW') === "true";
    }

    let request = new XMLHttpRequest();
    request.open('GET', `/api/config/random-slideshow`, false);
    request.send(null);

    return request.status === 200 && request.responseText === "true";
}

/**
 * Checks if images should be preloaded.
 * @returns {boolean} true if images should be preloaded, false otherwise
 */
function shouldPreloadImages() {
    let request = new XMLHttpRequest();
    request.open('GET', `/api/config/preload-images`, false);
    request.send(null);

    return request.status === 200 && request.responseText === "true";
}

/**
 * Initializes a new slideshow, if random is active fetch a random playlist.
 * Otherwise, fetch the current week's playlist.
 * If no images are available, fetch a random playlist.
 * If the slideshow is forced to be random, fetch a random playlist.
 */
function initSlideshow() {
    if (forceRandomSlideshow) {
        console.log("Forcing random slideshow");
        fetch('/api/resources/random')
            .then(response => response.json())
            .then(resources => beginSlideshow(resources))
            .catch(error => console.error('Error loading available images:', error));
    } else {
        fetch('/api/resources/week/count')
            .then(response => response.json())
            .then(count => {
                console.log("Available images this week:", count);
                if (count === 0) {
                    console.log("No images available, starting random slideshow");
                    return fetch('/api/resources/random')
                        .then(response => response.json())
                        .then(resources => beginSlideshow(resources));
                } else {
                    console.log("Starting this week's slideshow");
                    return fetch('/api/resources/week')
                        .then(response => response.json())
                        .then(resources => beginSlideshow(resources));
                }
            })
            .catch(error => console.error('Error loading available images:', error));
    }
}

/**
 * Starts the slideshow utilizing `setInterval`
 * The interval is set to the value returned from the backend API
 * @param foundResourcesOfThisWeek to start the slideshow with
 */
function beginSlideshow(foundResourcesOfThisWeek) {
    console.log("Starting slideshow with " + foundResourcesOfThisWeek.length + " images");

    resourcesThisWeek = foundResourcesOfThisWeek;

    maxIndex = Object.keys(resourcesThisWeek).length - 1;
    slideshowTick();

    // Load slideshow interval
    let intervalInSeconds = getSlideshowInterval();

    // Start image slideshow
    setInterval(() => slideshowTick(), intervalInSeconds * 1000);
}

/**
 * Checks if the hidden button should be visible and appends the click event listener on it
 */
function initHideButton() {
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.has('SHOW_HIDE_BUTTON')) {
        if (urlParams.get('SHOW_HIDE_BUTTON') === "true") {
            showHideButton();
        }
        return;
    }

    fetch(`/api/config/show-hide-button`)
        .then(response => response.json())
        .then(isHideButtonVisible => {
            if (isHideButtonVisible === true) {
                showHideButton();
            }
        })
        .catch(error => console.error("Error:", error));
}

function showHideButton() {
    let hideCurrentImageBtn = document.getElementById("hide-current-image");
    hideCurrentImageBtn.style.visibility = "visible";
    hideCurrentImageBtn.addEventListener("click", hideCurrentImage);
}

/**
 * Adds the current visible image to the hidden list and reloads the slideshow (to hide it)
 */
function hideCurrentImage() {
    fetch("/api/resources/hide/" + current_resource_id, {method: "POST"})
        .then(() => location.reload())
        .catch(error => console.error("Error:", error));
}


/**
 * Checks if the weather information should be shown, if so load them
 */
function loadWeatherInformation() {
    // First check if the WEATHER_ENABLED was overwritten in the url
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.has('WEATHER_ENABLED')) {
        if (urlParams.get('WEATHER_ENABLED') === "true") {
            loadCurrentWeather();
        }
        return;
    }

    // If not, check in the app config if weather is enabled
    fetch(`/api/weather`)
        .then(response => response.json())
        .then(showWeather => {
            if (showWeather === true) {
                loadCurrentWeather();
            }
        })
        .catch(error => console.error("Error:", error));
}

/**
 * Loads the current weather from the rest api and shows it
 */
function loadCurrentWeather() {
    fetch(`/api/weather/current`)
        .then(response => response.json())
        .then(data => {
            showCurrentWeather(data);
        })
        .catch(error => console.error("Error:", error));
}

/**
 * Shows the actual weather on the frontend.
 * If home assistant is enabled, the temperature is loaded from Home Assistant.
 * The weather icon is loaded from OpenWeatherMap.
 * @param data the weather data
 */
function showCurrentWeather(data) {
    const weather = data.weather[0];
    const icon = weather.icon;

    document.getElementById("weather-label").textContent = weather.description + ",";
    document.getElementById("weather-icon").src = `https://openweathermap.org/img/w/${icon}.png`;

    isHomeAssistantEnabled().then((enabled) => {
        if (enabled) {
            let homeAssistantData = JSON.parse(getCurrentTemperatureDataFromHomeAssistant());
            document.getElementById("weather-temperature").innerText =
                Math.round(homeAssistantData.state) + homeAssistantData.attributes.unit_of_measurement;
        } else {
            document.getElementById("weather-temperature").innerText = Math.round(data.main.temp) + "°C";
        }
    });
}

/**
 * @returns {Promise<boolean>} true if Home Assistant is enabled
 */
async function isHomeAssistantEnabled() {
    try {
        const response = await fetch(`/api/weather/homeassistant`);
        const data = await response.json();
        return data === true;
    } catch (error) {
        console.error("Error:", error);
        return false;
    }
}


/**
 * @returns {string} the current temperature from Home Assistant
 */
function getCurrentTemperatureDataFromHomeAssistant() {
    let request = new XMLHttpRequest();
    request.open('GET', `/api/weather/homeassistant/temperature`, false);
    request.send(null);
    if (request.status === 200) {
        return request.response;
    }
    return "{}";
}

/**
 * Sets the image url and its meta information to the frontend
 * This is done by fading out the current image and fading in the new image
 * The sleep function is used to prevent the slideshow from flickering
 * @param resource_id the id of the resource
 */
function setImage(resource_id) {
    console.log("Showing image: " + resource_id);

    // build the image url
    let screenWidth = window.screen.availWidth;
    let screenHeight = window.screen.availHeight;
    let imageUrl = `/api/resources/${resource_id}/${screenWidth}/${screenHeight}`;

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
        photoMetadataRequest.open("GET", "/api/resources/" + resource_id + "/description");
        photoMetadataRequest.send();
        photoMetadataRequest.onload = () => slideShowMetadata.innerText = photoMetadataRequest.response;

        // At last, set the current resource id
        current_resource_id = resource_id;
    })
}

/**
 * On slideshow tick interval.
 * Set the slideshow image and its meta information.
 */
function slideshowTick() {
    // Proceeds with the regular "this week" slideshow
    setImage(resourcesThisWeek[currentIndex]);

    currentIndex++;
    if (currentIndex > maxIndex) {
        currentIndex = 0;
    }

    // Preload next image if active
    if (shouldPreloadImages()) {
        preloadNextImage(resourcesThisWeek[currentIndex]);
    }
}

/**
 * Preloads the next image in the background, this done by requesting the image from the backend, because the backend caches the images.
 * Thus, consecutive requests for the same image are faster.
 * @param resource_id the id of the resource to preload
 */
function preloadNextImage(resource_id) {
    console.log("Preloading next image: " + resource_id);
    let screenWidth = window.screen.availWidth;
    let screenHeight = window.screen.availHeight;
    let request = new XMLHttpRequest();
    request.open("GET", `/api/resources/${resource_id}/${screenWidth}/${screenHeight}`);
    request.send();
}

/**
 * @returns {number} the slideshow interval in seconds
 */
function getSlideshowInterval() {
    // First check if the user overwrites the SLIDESHOW_INTERVAL as url parameter
    const urlParams = new URLSearchParams(window.location.search);
    if (urlParams.has('SLIDESHOW_INTERVAL')) {
        return parseInt(urlParams.get('SLIDESHOW_INTERVAL'))
    }

    // if no interval was found in the url, load the value from the config
    let request = new XMLHttpRequest();
    request.open('GET', `/api/config/interval/slideshow`, false);
    request.send(null);
    if (request.status === 200) {
        return parseInt(request.responseText);
    }
    return 30;
}

/**
 * @returns {number} the refresh interval in minutes from the backend API
 */
function getRefreshInterval() {
    let request = new XMLHttpRequest();
    request.open('GET', `/api/config/interval/refresh`, false);
    request.send(null);
    if (request.status === 200) {
        return parseInt(request.responseText)
    }
    return 180;
}

/**
 * Sleeps for the given amount of milliseconds and returns a promise that is resolved when the sleep is finished
 * @param ms the amount of milliseconds to sleep
 * @returns {Promise<unknown>}
 */
function sleep(ms) {
    return new Promise(resolver => setTimeout(resolver, ms));
}
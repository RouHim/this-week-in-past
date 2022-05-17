let resources;
let currentIndex = 0;
let maxIndex = 0;

window.onload = () => {
    loadAvailableImages();
    loadCurrentWeather();
    loadCurrentTempFromHomeAssistant();
};

function loadCurrentWeather() {
    const city = 'Koblenz';
    const app_id = '4021b60be2b322c8cfc749a6503bb553';
    const url = `https://api.openweathermap.org/data/2.5/weather?q=${city}&appid=${app_id}&units=metric&lang=de`;

    fetch(url)
        .then(response => response.json())
        .then(data => {
            const weather = data.weather[0];
            const icon = weather.icon;
            document.getElementById("weather-label").innerHTML = weather.description;
            document.getElementById("weather-icon").src = `https://openweathermap.org/img/w/${icon}.png`;
        });
}

function loadCurrentTempFromHomeAssistant() {
    const base_url = "http://192.168.0.5:8123";
    const token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiIzYzg5ZmY3MjMyZDg0ZmY2ODVkZDFkODhhOWQxYTRjMiIsImlhdCI6MTY0NTQ1OTA2MiwiZXhwIjoxOTYwODE5MDYyfQ.TknsqBMwriiE4_jrSjEi4z8vn0AUvLD8WYgL-BhaYKw";
    const entity_id = "sensor.aussen_sensor_temperature";
    const url = `${base_url}/api/states/${entity_id}`;

    fetch(url, {
        "method": "GET",
        "headers": {
            "Content-Type": "application/json",
            "Authorization": `Bearer ${token}`
        }
    })
        .then(response => response.json())
        .then(data => {
            document.getElementById("weather-temperature").innerText = Math.round(data.state) + "°C";
        });
}

function slideshowTick() {
    let photoDataRequest = new XMLHttpRequest();
    photoDataRequest.open("GET",
        `${window.location.href}api/resources/${resources[currentIndex]}/${window.screen.availWidth}/${window.screen.availHeight}/base64`
    );
    photoDataRequest.send();
    photoDataRequest.onload = () => document.getElementById("slideshow-image").src = photoDataRequest.response;

    let photoMetadataRequest = new XMLHttpRequest();
    photoMetadataRequest.open("GET", window.location.href + "api/resources/" + resources[currentIndex] + "/description");
    photoMetadataRequest.send();
    photoMetadataRequest.onload = () => document.getElementById("slideshow-metadata").innerText = photoMetadataRequest.response;

    currentIndex++;
    if (currentIndex > maxIndex) {
        currentIndex = 0;
    }
}

function startSlideshow(response) {
    resources = response;
    maxIndex = Object.keys(resources).length - 1;
    slideshowTick();

    // Tick every 10 seconds
    setInterval(() => slideshowTick(), 10000);

    // Reload every hour
    setInterval(() => location.reload(), 3600000);
}

function loadAvailableImages() {
    const http = new XMLHttpRequest();
    http.open("GET", window.location.href + "api/resources/week");
    http.send();
    http.responseType = "json"
    http.onload = () => startSlideshow(http.response);
}
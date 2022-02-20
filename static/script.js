let resources;
let currentIndex = 0;
let maxIndex = 0;

window.onload = () => {
    loadAvailableImages();
    loadCurrentWeather();
};

function loadCurrentWeather() {
    const url = 'https://api.openweathermap.org/data/2.5/weather?q=Koblenz&appid=4021b60be2b322c8cfc749a6503bb553&units=metric&lang=de';
    fetch(url)
        .then(response => response.json())
        .then(data => {
            const weather = data.weather[0];
            const temp = data.main.temp;
            const icon = weather.icon;
            document.getElementById("weather-temp").innerHTML = weather.description + ", " + Math.round(temp) + "°C";
            document.getElementById("weather-icon").src = "https://openweathermap.org/img/w/" + icon + ".png";
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
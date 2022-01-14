let images;
let currentIndex = 0;
let maxIndex = 0;

window.onload = () => {
    loadAvailableImages();
};

function slideshowTick() {
    let photoDataRequest = new XMLHttpRequest();
    photoDataRequest.open("GET", window.location.href + "api/photos/" + images[currentIndex]);
    photoDataRequest.send();
    photoDataRequest.onload = () => document.getElementById("slideshow-image").src = photoDataRequest.response;

    let photoMetadataRequest = new XMLHttpRequest();
    photoMetadataRequest.open("GET", window.location.href + "api/photos/" + images[currentIndex] + "/metadata");
    photoMetadataRequest.send();
    photoMetadataRequest.onload = () => document.getElementById("slideshow-metadata").innerText = photoMetadataRequest.response;

    currentIndex++;
    if (currentIndex > maxIndex) {
        currentIndex = 0;
    }
}

function startSlideshow(response) {
    images = response;
    maxIndex = Object.keys(images).length - 1;
    slideshowTick();

    // Tick every 10 seconds
    setInterval(() => slideshowTick(), 10000);

    // Reload every 6 hours
    setInterval(() => location.reload(), 21600000);
}

function loadAvailableImages() {
    const http = new XMLHttpRequest();
    http.open("GET", window.location.href + "api/photos");
    http.send();
    http.responseType = "json"
    http.onload = () => startSlideshow(http.response);
}
function changeSeenDisclaimer() {
    localStorage.setItem("accepted_explicit", true);
    document.getElementById('warn').style.display = "none";
    document.getElementById('main').style.display = "block";
}

window.addEventListener('DOMContentLoaded', () => {
    console.log("Start")
    if (localStorage["accepted_explicit"]) {
        document.getElementById('warn').style.display = "none";
        document.getElementById('main').style.display = "block";
    }
});

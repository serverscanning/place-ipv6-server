document.addEventListener("DOMContentLoaded", subscribeToCanvas);

function subscribeToCanvas() {
    const canvasImgEl = document.getElementById("canvas-img");
    const canvasStatusEl = document.getElementById("canvas-status");

    const evtSource = new EventSource("events");
    canvasStatusEl.innerText = "Connecting...";

    evtSource.onopen = (event) => {
        canvasStatusEl.innerText = "Connected";
    };
    evtSource.onerror = (event) => {
        canvasStatusEl.innerText = "Error. Attempting to reconnect in 3s...";
        setTimeout(subscribeToCanvas, 3000);
        evtSource.close();
    };
    evtSource.addEventListener("canvas_image", (event) => {
        canvasImgEl.setAttribute("src", event.data);
    });
}

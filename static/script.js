document.addEventListener("DOMContentLoaded", subscribeToCanvas);
document.addEventListener("DOMContentLoaded", subscribeToTabBecomingVisible);

let pendingImageUpdate = null;

function subscribeToTabBecomingVisible() {
    const canvasImgEl = document.getElementById("canvas-img");
    document.addEventListener("visibilitychange", (event) => {
        if (document.visibilityState === "visible" && pendingImageUpdate !== null) {
            canvasImgEl.setAttribute("src", pendingImageUpdate);
            pendingImageUpdate = null;
            console.log("Updated to pending image update because tab became visible again!");
        }
    });
}

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
        if (!document.hidden) {
            canvasImgEl.setAttribute("src", event.data);
        } else {
            pendingImageUpdate = event.data;
            //console.log("Saving update for later because tab is hidden!");
        }
    });
}

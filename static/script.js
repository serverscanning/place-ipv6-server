document.addEventListener("DOMContentLoaded", subscribeToCanvas);
//document.addEventListener("DOMContentLoaded", subscribeToTabBecomingVisible);

/*
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
}*/

function subscribeToCanvas() {
    const canvasEl = document.getElementById("canvas");
    const canvasCtx = canvasEl.getContext("2d");
    const canvasStatusEl = document.getElementById("canvas-status");

    console.log("Websocket: Connecting...");
    canvasStatusEl.innerText = "Connecting...";

    const ws = new WebSocket((document.location.protocol === "https:" ? "wss://" : "ws://") + document.location.host + "/ws");
    ws.binaryType = "blob";
    ws.onopen = (event) => {
        console.log("Websocket: Connected");
        canvasStatusEl.innerText = "Connected";
    };

    let didError = false;
    ws.onerror = (event) => {
        console.log("Websocket: Error!");
        didError = true;
        ws.close();
    };

    ws.onclose = (event) => {
        console.log("Websocket: Closed. Reconnecting in 3s...");
        canvasStatusEl.innerText = (didError ? "Error!" : "Lost connection!") + " Attempting to reconnect in 3s...";
        setTimeout(subscribeToCanvas, 3000);
    }

    ws.onmessage = async (event) => {
        if (!(event.data instanceof Blob)) return; // Receiving text is not supported rn (pps later?)
        let imageBitmap = await createImageBitmap(event.data);
        canvasCtx.drawImage(imageBitmap, 0, 0);
    }
}

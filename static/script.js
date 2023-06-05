document.addEventListener("DOMContentLoaded", subscribeToCanvas);

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

        ws.send(JSON.stringify({ request: "delta_canvas_stream", enabled: true }));
        ws.send(JSON.stringify({ request: "get_full_canvas_once" }));
    };

    const visibilityChangeHandler = (event) => {
        if (document.visibilityState === "visible") {
            ws.send(JSON.stringify({ request: "delta_canvas_stream", enabled: true }));
            ws.send(JSON.stringify({ request: "get_full_canvas_once" }));
            console.log("Document became visible again. Enabled receiving delta frames again and requested a full canvas.");
        } else {
            ws.send(JSON.stringify({ request: "delta_canvas_stream", enabled: false }));
            console.log("Document invisible. Disabled receiving delta frame updates.");
        }
    };
    document.addEventListener("visibilitychange", visibilityChangeHandler);

    let didError = false;
    ws.onerror = (event) => {
        console.log("Websocket: Error!");
        didError = true;
        ws.close();
    };

    ws.onclose = (event) => {
        document.removeEventListener("visibilitychange", visibilityChangeHandler);
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

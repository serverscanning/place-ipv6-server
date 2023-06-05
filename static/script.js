document.addEventListener("DOMContentLoaded", subscribeToCanvas);

function subscribeToCanvas() {
    const canvasEl = document.getElementById("canvas");
    const canvasCtx = canvasEl.getContext("2d");
    const canvasStatusEl = document.getElementById("canvas-status");
    const canvasPpsEl = document.getElementById("canvas-pps");

    console.log("Websocket: Connecting...");
    canvasStatusEl.innerText = "Connecting...";
    canvasPpsEl.innerText = "";

    const ws = new WebSocket((document.location.protocol === "https:" ? "wss://" : "ws://") + document.location.host + "/ws");
    ws.binaryType = "blob";
    ws.onopen = (event) => {
        console.log("Websocket: Connected");
        canvasStatusEl.innerText = "Connected";

        ws.send(JSON.stringify({ request: "delta_canvas_stream", enabled: true }));
        ws.send(JSON.stringify({ request: "get_full_canvas_once" }));
        ws.send(JSON.stringify({ request: "pps_updates", enabled: true }));
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
        canvasPpsEl.innerText = "";
        setTimeout(subscribeToCanvas, 3000);
    }

    let ppsEntries = [];

    ws.onmessage = async (event) => {
        if (event.data instanceof Blob) {
            let imageBitmap = await createImageBitmap(event.data);
            canvasCtx.drawImage(imageBitmap, 0, 0);
        } else if(typeof(event.data) === "string") {
            let wsMessage = JSON.parse(event.data);
            if (wsMessage.message === "pps_update") {
                ppsEntries.push(wsMessage.pps);
                while (ppsEntries.length > 10) ppsEntries.splice(0, 1);
                let maxPps = 0;
                for (const pps of ppsEntries) if (pps > maxPps) maxPps = pps;
                canvasPpsEl.innerText = "PPS (current / max in last 30s): " + formatNumber(wsMessage.pps, digits(maxPps)) + " / " + formatNumber(maxPps, 0);
            }
        } else {
            console.error("Received invalid type ws data: " + typeof (event.data));
        }
    }
}

function digits(number) {
    return Math.floor(Math.log10(number) + 1);
}

function formatNumber(number, padToMinDigits) {
    let numberStrRev = (number + "").split("").reverse().join("");
    while (numberStrRev.length < padToMinDigits) numberStrRev += "0";
    let newNumberStrRev = "";
    for (const i in numberStrRev) {
        if (i > 0 && i % 3 == 0) newNumberStrRev += ",";
        newNumberStrRev += numberStrRev[i];
    }
    return newNumberStrRev.split("").reverse().join("");
}

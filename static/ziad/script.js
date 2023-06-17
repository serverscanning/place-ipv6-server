function changeSeenDisclaimer() {
  localStorage.setItem("accepted_explicit", true);
  document.getElementById('warn').style.display = "none";
  document.getElementById('main').style.display = "block";
}

document.addEventListener("DOMContentLoaded", subscribeToCanvas);

maxPps = 0;
graphCounter = 0;

function tmGraph(cvs2, w, h, bs, bsy, gf, fg, df, dataFunction, gData = [], zeroGdata = false){
    let WIDTH = cvs2.width = w;
    let HEIGHT = cvs2.height = h;
    const DIFF = df;
    let BOX_SIZE = bs;
    let BOX_SIZEY = bsy;
    const GRAPHCOLOR = gf;
    const LINECOLOR = fg;

    cvs2.style.imageRendering = "pixelated";
    let ctx2 = cvs2.getContext('2d');

    let graphHistory = [...gData];
    if (graphHistory.length < (WIDTH/DIFF)+10 && zeroGdata){
        let amt = Math.round((WIDTH/DIFF)+10);
        graphHistory = [...(new Array(amt)).fill(1), ...gData]
    }

    function pushGraphHistory(y){
        graphHistory.push(y);
        if (graphHistory.length > (WIDTH/DIFF)+10) graphHistory.shift();
    }

    function getLine(x1, y1, x2, y2) {
        let coords = new Array();
        let dx = Math.abs(x2 - x1);
        let dy = Math.abs(y2 - y1);
        let sx = (x1 < x2) ? 1 : -1;
        let sy = (y1 < y2) ? 1 : -1;
        let err = dx - dy;
        coords.push([x1, y1]);
        while (!((x1 == x2) && (y1 == y2))) {
            let e2 = err << 1;
            if (e2 > -dy) {
                err -= dy;
                x1 += sx;
            }
            if (e2 < dx) {
                err += dx;
                y1 += sy;
            }
            coords.push([x1, y1]);
        }
        return coords;
    }

    function drawCoordsArr(arr){
        for (let a of arr){
            ctx2.fillRect(a[0], a[1], 1, 1);
        }
    }

    function draw(){
        // Add data
        let data = dataFunction(graphCounter);
        pushGraphHistory(data);

        // Clear graph
        ctx2.fillStyle = "#000000";
        ctx2.fillRect(0, 0, WIDTH, HEIGHT);
        graphCounter++;

        // Draw graph paper
        ctx2.fillStyle = GRAPHCOLOR;
        for (let i = 0; i < HEIGHT; i++){
            (i + 1) % BOX_SIZEY === 0 && i != HEIGHT - 1 && ctx2.fillRect(0, i, WIDTH, 1);
        }
        for (let i = 0; i < WIDTH; i++){
            (i + graphCounter * DIFF) % BOX_SIZE === 0 && ctx2.fillRect(i, 0, 1, HEIGHT);
        }

        // Draw graph lines
        ctx2.fillStyle = LINECOLOR;
        let rVal = null;
        graphHistory.reverse();
        for (let a in graphHistory){
            let x = WIDTH - ((a) * DIFF);
            if (x <= 2)
                continue;
            let val = graphHistory[a];
            let valA = HEIGHT - Math.floor(val / (maxPps + ~~(maxPps/10)) * HEIGHT);
            if (isNaN(valA) || (valA >= HEIGHT) || (valA < 0))
                valA = HEIGHT - 1;
            rVal && drawCoordsArr(getLine(rVal[0], rVal[1], x, valA));
            rVal = [x, valA];
        }
        graphHistory.reverse();
    }

    return draw;
}

function subscribeToCanvas() {
    const canvasEl = document.getElementById("cvs");
    const canvasCtx = canvasEl.getContext("2d");
    const canvasPpsEl = document.getElementById("pps");
    let dr = false;

    console.log("Websocket: Connecting...");
    canvasPpsEl.innerText = "Connecting...";
    const ws = new WebSocket((document.location.protocol === "https:" ? "wss://" : "ws://") + document.location.host + "/ws");
    ws.binaryType = "blob";
    ws.onopen = (event) => {
        console.log("Websocket: Connected");
        canvasPpsEl.innerText = "Connected";

        ws.send(JSON.stringify({ request: "delta_canvas_stream", enabled: true }));
        ws.send(JSON.stringify({ request: "get_full_canvas_once" }));
        ws.send(JSON.stringify({ request: "pps_updates", enabled: true }));

        dr = tmGraph(document.querySelector('#c6'), 800, 79, 25, 25, "#008040", "lime", 2, (gc)=>{
                return maxPps;
        }, [], true);

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
        canvasPpsEl.innerText = (didError ? "Error!" : "Lost connection!") + " Attempting to reconnect in 3s...";
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
                while (ppsEntries.length > 10)
                    ppsEntries.splice(0, 1);
                maxPps = 0;
                for (const pps of ppsEntries)
                    if (pps > maxPps)
                        maxPps = pps;
                canvasPpsEl.innerHTML = "PPS </br>" + formatNumber(wsMessage.pps, digits(maxPps));
                if (dr) dr();
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

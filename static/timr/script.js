document.addEventListener('DOMContentLoaded', subscribeToCanvas);

pps = 0;
graphCounter = 0;
user_id = null;
user_ip = null;

async function getID() {
    const id = await (await fetch('/my_user_id')).json();
    if (id['ip'] !== null && id['user_id'] !== null) {
        user_id = id['user_id'];
        user_ip = id['ip'];
    } else {
        user_id = null;
        user_ip = null;
    }
    console.log('my_user_id: ' + user_id + ' from ' + user_ip);
}

function canvasXY() {
    const canvasEl = document.getElementById('canvas');
    canvasEl.addEventListener('mousemove', (e) => {
        document.getElementById('ipv6-size').innerText = '1';
        document.getElementById('ipv6-coords-x').innerText = e.offsetX.toString(16).padStart(3,'0');
        document.getElementById('ipv6-coords-y').innerText = e.offsetY.toString(16);
        pixel = document.getElementById('canvas').getContext('2d').getImageData(e.offsetX, e.offsetY, 1, 1).data;
        document.getElementById('ipv6-colors-red').innerText = pixel[0].toString(16);
        document.getElementById('ipv6-colors-green').innerText = pixel[1].toString(16);
        document.getElementById('ipv6-colors-blue').innerText = pixel[2].toString(16).padStart(2,'0');
        document.getElementById('canvas').setAttribute( 'title', 'Canvas: ' +
		e.offsetX + 'x' + e.offsetY + ' #' +
	        pixel[0].toString(16).padStart(2,'0') +
	        pixel[1].toString(16).padStart(2,'0') +
	        pixel[2].toString(16).padStart(2,'0'));
    });
    canvasEl.addEventListener('mouseout', (e) => {
        document.getElementById('canvas').setAttribute( 'title', 'Canvas');
        document.getElementById('ipv6-coords-x').innerText = 'XXX';
        document.getElementById('ipv6-coords-y').innerText = 'YYYY';
        document.getElementById('ipv6-size').innerText = 'S';
        document.getElementById('ipv6-colors-red').innerText = 'RR';
        document.getElementById('ipv6-colors-green').innerText = 'GG';
        document.getElementById('ipv6-colors-blue').innerText = 'BB';
    });
}

async function resizeCanvas() {
    const serverConfig = await (await fetch('/serverconfig.json')).json();
    if (serverConfig['width'] !== null && serverConfig['height'] !== null) {
         document.getElementById('canvas').width = serverConfig['width'];
         document.getElementById('canvas').height = serverConfig['height'];
         document.getElementById('ipv6-coords').setAttribute( 'title',
                 'The X and Y coordinates of the pixel(s) you want to change. ' +
                 'Canvas resolution is ' +
                 serverConfig['width'] + 'x' +
                 serverConfig['height']);
         console.log('Canvas sized to ' + serverConfig['width'] + 'x' + serverConfig['height']);
    } else {
        console.log('No width/height in serverconfig.json!');
    }
}

function tmGraph(cvs2, w, h, bs, bsy, gf, fg, df, dataFunction){
    let WIDTH = cvs2.width = w;
    let HEIGHT = cvs2.height = h;
    const DIFF = df;
    let BOX_SIZE = bs;
    let BOX_SIZEY = bsy;
    const GRAPHCOLOR = gf;
    const LINECOLOR = fg;

    cvs2.style.imageRendering = 'pixelated';
    let ctx2 = cvs2.getContext('2d');

    let graphHistory = (new Array(WIDTH)).fill([0,0]);

    function pushGraphHistory(y){
        graphHistory.push(y);
        if (graphHistory.length > WIDTH)
            graphHistory.shift();
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
        ctx2.fillStyle = '#000000';
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
        let rVal1 = null;
        let rVal2 = null;
        let maxPps = 0;
        let maxWs = 10
        for (const graphEntry of graphHistory) {
            if (graphEntry[0] > maxPps)
                maxPps = graphEntry[0];
            if (graphEntry[1] > maxWs)
                maxWs = graphEntry[1];
        }
        graphHistory.reverse();
        for (let a in graphHistory) {
            ctx2.fillStyle = 'teal';
            let x2 = WIDTH - ((a) * DIFF);
            if (x2 <= 2)
                continue;
            let val = graphHistory[a][1];
            let valA = HEIGHT - Math.floor(val / (maxWs + ~~(maxWs/10)) * HEIGHT);
            if (isNaN(valA) || (valA >= HEIGHT) || (valA < 0))
                valA = HEIGHT - 1;
            rVal2 && drawCoordsArr(getLine(rVal2[0], rVal2[1], x2, valA));
            rVal2 = [x2, valA];

            ctx2.fillStyle = 'red';
            let x1 = WIDTH - ((a) * DIFF);
            if (x1 <= 2)
                continue;
            val = graphHistory[a][0];
            valA = HEIGHT - Math.floor(val / (maxPps + ~~(maxPps/10)) * HEIGHT);
            if (isNaN(valA) || (valA >= HEIGHT) || (valA < 0))
                valA = HEIGHT - 1;
            rVal1 && drawCoordsArr(getLine(rVal1[0], rVal1[1], x1, valA));
            rVal1 = [x1, valA];
        }
        graphHistory.reverse();
    }

    return draw;
}

function subscribeToCanvas() {
    const canvasEl = document.getElementById('canvas');
    const canvasCtx = canvasEl.getContext('2d', { willReadFrequently: true });
    const canvasPpsEl = document.getElementById('canvas-pps');
    let dr = false;

    console.log('Websocket: Connecting...');
    canvasPpsEl.innerText = 'Connecting...';
    resizeCanvas();
    canvasXY();
    getID();

    const ws = new WebSocket((document.location.protocol === 'https:' ? 'wss://' : 'ws://') + document.location.host + '/ws');
    ws.binaryType = 'blob';
    ws.onopen = (event) => {
        console.log('Websocket: Connected');
        canvasPpsEl.innerText = 'Connected';

        ws.send(JSON.stringify({ request: 'nudity_updates', enabled: true }));
        ws.send(JSON.stringify({ request: 'get_nudity_update_once' }));
        ws.send(JSON.stringify({ request: 'delta_canvas_stream', enabled: true }));
        ws.send(JSON.stringify({ request: 'get_full_canvas_once' }));
        ws.send(JSON.stringify({ request: 'pps_updates', enabled: true }));
        ws.send(JSON.stringify({ request: 'ws_count_updates', enabled: true }));
        ws.send(JSON.stringify({ request: 'get_ws_count_update_once' }));

        dr = tmGraph(document.querySelector('#c6'), 800, 79, 25, 25, '#008040', 'red', 2, (gc)=>{
                return [pps, ws_count];
        }, [], true);

    };

    const visibilityChangeHandler = (event) => {
        if (document.visibilityState === 'visible') {
            ws.send(JSON.stringify({ request: 'delta_canvas_stream', enabled: true }));
            ws.send(JSON.stringify({ request: 'get_full_canvas_once' }));
            console.log('Document became visible again. Enabled receiving delta frames again and requested a full canvas.');
        } else {
            ws.send(JSON.stringify({ request: 'delta_canvas_stream', enabled: false }));
            console.log('Document invisible. Disabled receiving delta frame updates.');
        }
    };
    document.addEventListener('visibilitychange', visibilityChangeHandler);

    let didError = false;
    ws.onerror = (event) => {
        console.log('Websocket: Error!');
        didError = true;
        ws.close();
    };

    ws.onclose = (event) => {
        document.removeEventListener('visibilitychange', visibilityChangeHandler);
        console.log('Websocket: Closed. Reconnecting in 3s...');
        canvasPpsEl.innerText = (didError ? 'Error!' : 'Lost connection!') + ' Attempting to reconnect in 3s...';
        setTimeout(subscribeToCanvas, 3000);
    }

    let ppsEntries = [];
    let ws_count = 1;
    let ws_max = ws_count;
    let width = document.querySelector('#c6').width;

    ws.onmessage = async (event) => {
        if (event.data instanceof Blob) {
            let imageBitmap = await createImageBitmap(event.data);
            canvasCtx.drawImage(imageBitmap, 0, 0);
        } else if(typeof(event.data) === 'string') {
            let wsMessage = JSON.parse(event.data);
            //console.log(wsMessage)
            if (wsMessage.message === 'pps_update') {
                ppsEntries.push(wsMessage.pps);
                pps = wsMessage.pps;
                while (ppsEntries.length > width)
                    ppsEntries.splice(0, 1);
                let maxPps = 0;
                for (const ppsEntry of ppsEntries)
                    if (ppsEntry > maxPps)
                        maxPps = ppsEntry;
                canvasPpsEl.innerHTML = 'Current / Max: </br>' +
                        '<div style="color:red;">PPS: ' + formatNumber(pps, digits(maxPps)) +
                        ' / ' + formatNumber(maxPps, 0) + '</div>' +
                        '<div style="color:teal;">Viewers: ' + ws_count + ' / ' + ws_max + '</div>';
                if (dr) dr();
            } else if (wsMessage.message === 'ws_count_update') {
                ws_count = wsMessage.ws_connections;
                if (ws_count > ws_max)
                    ws_max = ws_count;
                //console.log('WSC: ' + ws_count + ' / ' + ws_max);
            } else if (wsMessage.message === 'nudity_update') {
                if (wsMessage.is_nude) {
                    console.log("WARN: The server reports the image is/became nude!");
                    censorCanvas("The server detected nudity!");
                } else {
                    console.log("The server reports the image not nude (anymore).");
                    uncensorCanvas();
                }
            }
        } else {
            console.error('Received invalid type ws data: ' + typeof (event.data));
        }
    }
}

function digits(number) {
    return Math.floor(Math.log10(number) + 1);
}

function formatNumber(number, padToMinDigits) {
    let numberStrRev = (number + '').split('').reverse().join('');
    while (numberStrRev.length < padToMinDigits) numberStrRev += '0';
    let newNumberStrRev = '';
    for (const i in numberStrRev) {
        if (i > 0 && i % 3 == 0) newNumberStrRev += ',';
        newNumberStrRev += numberStrRev[i];
    }
    return newNumberStrRev.split('').reverse().join('');
}

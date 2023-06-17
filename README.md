# IPv6-Place-v2

A re-implementation of ziad87's awesome "Place: IPv6" site.

Difference to the original is, that this only needs a /64-IPv6 block instead of a /48 one. Everything is pushed one segment back and GG+BB share the last segment now (for the full impl, look for the function "from_addr" in `src/ping_receiver.rs`).

![Screenshot](https://transfer.cosmos-ink.net/hHufof4KOC/grafik.png)

## Backend

Receives Pings to any prefix on the interface it was told to listen on and draws pixels on the canvas accordingly.

The canvas is available to be requested at `/canvas.png` or via the Websocket (`/ws`).

### Websocket

The Websocket is the main method used to interact with the webserver.

When connecting to the Websocket, by default it will not send you any data and just stay open. Currently the websocket accepts these 3 commands, which are parsed as `WsRequest` in `src/main.rs`:

- `{ "request": "get_full_canvas_once" }`: Return a binary message once containing the full canvas (RGB-png file)
- `{ "request": "delta_canvas_stream", "enabled": <bool> }`: Turn on receiving delta frames (binary messages) when pings are received (off by default, RGBA-png files)
- `{ "request": "pps_updates", "enabled": <bool> }`: Turn on receiving pps updates every second (text message like this: `{ "message": "pps_update", "pps" <number> }`)
- `{ "request": "get_ws_count_update_once" }`: Receive a WS Count Update once (text message like this: `{ "message": "ws_count_update", "ws_connections" <number> }`)
- `{ "request": "ws_count_updates", "enabled": <bool> }`: Enable receiving ws count updates when it changes. Messages will look the same as for `get_ws_count_update_once`

## Frontend

Any non-declared routes (currently `/ws` and `/canvas.png`) will be served from the `static/` folder. So the frontend lives here and can be implemented with any means necessary so long as it uses the websocket to receive data.

Currently the default route (`/` aka `/index.html`) will serve the default frontend for this server as shown in the screenshot above.

[Sudo-null7](https://github.com/Sudo-null7) also contributed a variant/theme that looks more like [the old interface that ziad87 used](https://i.xevion.dev/2023/03/firefox_UMf1xj8hrL.png) in the end. It can be shown by navigating to `/ziad` (aka `/ziad/index.html`).

## Hosted site

This server is currently hosted at this address: <http://[2a01:4f8:c012:f8e6::1]/>

Not everyone has native IPv6 support at home though sadly. So there are a number of proxies that show the site (mostly using Cloudflares webproxy):

- <https://ek98.casper.wf> (hosted by someone taking part in the old version of ziad87)
- <https://v6.ssi.pet/> (hosted by a member of the SSI discord)
- <https://ssi.place> (hosted by me, EnderKill98)

(I've kept the owners vague here just to be sure. Feel free to DM me and I'll add any full reference as you like).

### Getting started

To get started, you can experiment pinging the mentioned IPv6 with e.g. the `ping` tool of your OS like this: `ping 2a01:4f8:c012:f8e6:200a:a:ff:0000` (should make a red 2x2 pixel appear at x: 10, y: 10). The format and usage are displayed on the site itself.

There are many libraries for languages to speed up pinging in different ways. A fun challenge is to try an image onto the canvas using pings. Videos are certainly also possible with a powerful server.

It is generally recommended to use a VPS or other kind of server to run pingers that are any faster than about a dozen pings / sec because most ISPs filter more pings heavily. The current server has been observed to be able to a bit over 630k pings/sec, but doing such massive ping floods could make your ISP believe that you try to DOS someone. So ping very fast at your own risk (you could receive abuse complains from e.g. your hoster for it). So far no abuse complaint has been received on the hosting site for now, but please do not use your productively used instance if you're not sure.

## What is possible

When making a client pretty efficient, playing video like this (depending on the implementation and tuning even cleaner) is possible:

<https://github.com/serverscanning/place-ipv6-server/assets/117233381/8934f101-95f6-488f-9715-fd4a7866cfb5>

## TODO

- [ ] Add more helpful links?
- [ ] Add example on how to easily self-host this locally

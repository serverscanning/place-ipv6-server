# IPv6-Place-v2

A re-implementation of ziad87's awesome "Place: IPv6" site.

Written in Rust with Axum using [Server-Sent-Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events) for the fun instead of WS (maybe also add that for bots and such).

Difference to the original is, that this only needs a /64-IPv6 block instead of a /48 one. Everything is pushed one segment back and GG+BB share the last segement now (for the full impl, look for the function "from_addr" in `src/ping_receiver.rs`).

TODO:

- [ ] Use Infinite Loop instead of `receive()` for potentially better performance and less loss
- [ ] Improve Rate limit. Currently the last pings will only show up if another ping arrives.
- [ ] Make a decent looking website
- [ ] Host it somewhere

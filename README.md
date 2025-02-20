# reipc
This adds support for the [JSON-RPC 2.0](https://www.jsonrpc.org/specification) request/response communication style for IPC, specifically over [Unix Domain Sockets(UDS)](https://en.wikipedia.org/wiki/Unix_domain_socket).
The UDS supports full-duplex communication, so to leverage this, the R/W operations occur concurrently (each in its own OS thread).

# IMPORTANT 
This is alpha-level quality. I wanted this ASAP, so it is not up to _the standards_.
Not well tested.
If we realize this is useful, might nuke this specific repo and do it from scratch properly :)

# HIGH-LEVEL OVERIVEW
![reipc_high_level_overivew](https://github.com/user-attachments/assets/e654e5b7-eb71-4c69-9756-cd98ac461af6)



### Dependencies
Uses [alloy.rs](https://github.com/alloy-rs/alloy) for JSON-RPC 2.0 implementation ([alloy-json-rpc](https://github.com/alloy-rs/alloy/tree/main/crates/json-rpc)).

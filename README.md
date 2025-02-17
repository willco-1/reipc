# reipc

Adds support for [JSON-RPC 2.0](https://www.jsonrpc.org/specification) request/response communication style for IPC, specifically over [Unix Domain Sockets(UDS)](https://en.wikipedia.org/wiki/Unix_domain_socket).
The UDS support full duplex communication, so to leverage this, the R/W operations occur concurrently (each in its own OS thread).


### Dependencies
Uses [alloy.rs] (https://github.com/alloy-rs/alloy) for JSON-RPC 2.0 implementation ([alloy-json-rpc](https://github.com/alloy-rs/alloy/tree/main/crates/json-rpc)).

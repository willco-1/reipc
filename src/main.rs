use std::{env, path::Path};

use alloy_rpc_types_eth::{Block, BlockNumberOrTag};
use reipc::rpc_reqresp::RpcRequest;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("Usage: reipc <ipc_path> or cargo run -- <ipc_pah>");
    }

    let ipc_path = &args[1];
    let ipc = reipc::ReIPC::try_connect(Path::new(ipc_path))?;

    let rpc_request = RpcRequest::default();
    let req = rpc_request.make_request_empty_params("eth_blockNumber");
    let resp = RpcRequest::parse_response::<BlockNumberOrTag>(ipc.call(req)?)?;

    let req = rpc_request.make_request("eth_getBlockByNumber", (resp, true));
    let resp = ipc.call(req)?;
    println!("{:?}", RpcRequest::parse_response::<Block>(resp)?);

    ipc.close()?;
    Ok(())
}

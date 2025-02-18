use std::{env, path::Path};

use alloy_rpc_types_eth::{Block, BlockNumberOrTag};
use reipc::rpc_provider::RpcProvider;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("Usage: reipc <ipc_path> or cargo run -- <ipc_pah>");
    }

    let ipc_path = &args[1];
    let ipc = reipc::re_ipc_transport::ReIPC::try_connect(Path::new(ipc_path))?;

    let rpc_provider = RpcProvider::new(ipc);
    let block_num = rpc_provider.call_no_params::<BlockNumberOrTag>("eth_blockNumber")?;
    println!("{:?}", block_num);

    let block = rpc_provider.call::<_, Block>("eth_getBlockByNumber", (block_num, true))?;
    println!("{:?}", block);

    rpc_provider.close()?;
    Ok(())
}

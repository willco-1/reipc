use std::{env, path::Path};

use alloy_rpc_types_eth::{Block, BlockNumberOrTag};
use reipc::RpcProvider;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        let msg =
            "Usage: reipc <ipc_path> or cargo run -- <ipc_pah>\nExample: cargo run -- ../nmc.ipc";

        panic!("{msg}");
    }

    let rpc_provider = RpcProvider::try_connect(Path::new(&args[1]))?;
    let block_num = rpc_provider.call_no_params::<BlockNumberOrTag>("eth_blockNumber")?;
    println!("{:?}", block_num);

    let block = rpc_provider.call::<_, Block>("eth_getBlockByNumber", (block_num, true))?;
    println!("{:?}", block);

    rpc_provider.close()?;
    Ok(())
}

use std::{env, fmt::Debug, path::Path, str::FromStr, thread::JoinHandle, time::Duration};

use alloy_primitives::Address;
use alloy_rpc_types_eth::{Block, BlockNumberOrTag, EIP1186AccountProofResponse};
use reipc::{errors::RpcError, rpc_provider::RpcProvider};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        let msg =
            "Usage: reipc <ipc_path> <test_thread_count>or cargo run -- <ipc_pah> <test_thread_count>\nExample: cargo run -- ../nmc.ipc 10";

        panic!("{msg}");
    }

    let limit = if args.len() < 3 {
        4
    } else {
        args[2].parse::<usize>()?
    };

    let rpc_provider =
        RpcProvider::try_connect(Path::new(&args[1]), Duration::from_millis(50).into())?;

    let mut jhs = vec![];
    for _ in 0..limit {
        let jh = execute_call_in_thread::<_, Block>(
            rpc_provider.clone(),
            "eth_getBlockByNumber".into(),
            (BlockNumberOrTag::Latest, true),
        );
        jhs.push(jh);
    }

    let jh2 = execute_call_in_thread::<_, EIP1186AccountProofResponse>(
        rpc_provider.clone(),
        "eth_getProof".into(),
        (
            Address::from_str("0xe5cB067E90D5Cd1F8052B83562Ae670bA4A211a8")?,
            (),
            BlockNumberOrTag::Latest,
        ),
    );

    jhs.into_iter().for_each(|jh| {
        if let Err(e) = jh.join().unwrap() {
            println!("{e:?}");
        }
    });
    let _ = jh2.join().unwrap();
    if let Err(e) = rpc_provider.close() {
        println!("{e:?}");
    }

    Ok(())
}

fn execute_call_in_thread<Params, Resp>(
    rpc_provider: RpcProvider,
    method: String,
    params: Params,
) -> JoinHandle<Result<(), RpcError>>
where
    Params: alloy_json_rpc::RpcSend + 'static,
    Resp: Debug + serde::de::DeserializeOwned,
{
    std::thread::spawn(move || -> Result<(), RpcError> {
        let resp = rpc_provider.call::<Params, Resp>(method, params)?;
        let separator = "===============================================================";
        println!("{:?}\n{separator}\n{separator}", resp);
        Ok(())
    })
}

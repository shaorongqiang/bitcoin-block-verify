use std::env::set_var;

use bitcoincore_rpc::{Auth, Client, RpcApi};
use methods::{BITCOIN_BLOCK_VERIFY_ELF, BITCOIN_BLOCK_VERIFY_ID};
use risc0_zkvm::{default_prover, sha::Digestible, ExecutorEnv};

fn main() {
    env_logger::init();
    let mut data = Vec::new();
    for i in BITCOIN_BLOCK_VERIFY_ID {
        data.extend(i.to_le_bytes());
    }
    let input = {
        let mut input = Vec::new();
        let url = "http://127.0.0.1:18443";
        let auth = Auth::UserPass("admin1".into(), "123".into());

        let client = Client::new(url, auth).unwrap();

        let begin = 10;
        let end = 15;
        for height in begin..=end {
            let header = client
                .get_block_hash(height)
                .and_then(|hash| client.get_block_hex(&hash))
                .unwrap();
            let data = hex::decode(&header).unwrap();
            input.extend_from_slice(&data[..80]);
        }
        (end, input)
    };

    let env = ExecutorEnv::builder()
        .write(&input)
        .unwrap()
        .build()
        .unwrap();

    //set_var("RISC0_PROVER", "ipc");
    let prover = default_prover();
    //println!("{}", prover.get_name());

    let receipt = prover.prove(env, BITCOIN_BLOCK_VERIFY_ELF).unwrap();
    //println!("{:?}", receipt);

    let (height, hash) = receipt.journal.decode::<(u64, [u8; 32])>().unwrap();
    println!("output: {} 0x{}", height, hex::encode(hash));

    receipt.verify(BITCOIN_BLOCK_VERIFY_ID).unwrap();
    println!(
        "postStateDigest:{}",
        receipt
            .get_claim()
            .unwrap()
            .post
            .as_value()
            .unwrap()
            .digest()
    );
    match receipt.inner {
        risc0_zkvm::InnerReceipt::Composite(_receipt) => {}
        risc0_zkvm::InnerReceipt::Succinct(receipt) => {
            println!("Succinct:{}", hex::encode(receipt.get_seal_bytes()));
        }
        risc0_zkvm::InnerReceipt::Compact(receipt) => {
            println!("Compact:{}", hex::encode(receipt.seal));
        }
        risc0_zkvm::InnerReceipt::Fake { claim: _ } => {}
    }
}

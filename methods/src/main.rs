// Copyright 2023 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{env, io, io::Write, path::Path, time::Duration};

use anyhow::{Context, Result};
use bonsai_starter_methods::guest_list::{GuestEntry, GUEST_LIST};
use clap::Parser;
use risc0_recursion::SessionRollupReceipt;
use risc0_zkvm::{serde::to_vec, Executor, ExecutorEnv};

/// Runs the RISC-V ELF binary.
#[derive(Parser)]
#[clap(about, version, author)]
struct Args {
    /// The name of the guest binary
    guest_binary: String,

    /// The input to provide to the guest binary
    input: Option<String>,
}

fn prove_locally(guest_entry: &GuestEntry, input: Vec<u8>) -> Vec<u8> {
    let env = ExecutorEnv::builder()
        .add_input(&to_vec(&input).expect("Failed to vectorize data"))
        .build();
    let mut exec =
        Executor::from_elf(env, guest_entry.elf).expect("Failed to instantiate executor");
    let session = exec.run().expect("Failed to run executor");
    // Locally prove resulting journal
    if env::var("PROVE_LOCALLY").is_ok() {
        session.prove().expect("Failed to prove session");
    }
    session.journal
}

fn prove_remote(guest_entry: &GuestEntry, input: Vec<u8>) -> Result<Vec<u8>> {
    let client =
        bonsai_sdk::Client::from_env().context("Failed to initialize bonsai from env vars")?;
    let elf_path = Path::new(guest_entry.path);
    let img_id = client.upload_img_file(elf_path)?;
    let input_id = client.upload_input(input)?;
    let session = client.create_session(img_id, input_id)?;

    loop {
        let res = session.status(&client)?;
        if res.status == "RUNNING" {
            std::thread::sleep(Duration::from_secs(15));
            continue;
        }
        if res.status == "SUCCEEDED" {
            let receipt_url = res
                .receipt_url
                .context("API error, missing receipt on completed session")?;

            let receipt_buf = client.download(&receipt_url)?;
            let receipt: SessionRollupReceipt = bincode::deserialize(&receipt_buf)?;
            receipt
                .verify(guest_entry.image_id)
                .context("Receipt verification failed")?;
            return Ok(receipt.journal);
        } else {
            panic!("Workflow exited: {}", res.status);
        }
    }
}

pub fn main() {
    // Parse arguments
    let args = Args::parse();
    // Search list for requested binary name
    let potential_guest_image_id: [u8; 32] =
        match hex::decode(args.guest_binary.to_lowercase().trim_start_matches("0x")) {
            Ok(byte_vector) => byte_vector.try_into().unwrap_or([0u8; 32]),
            Err(_) => [0u8; 32],
        };
    let guest_entry = GUEST_LIST
        .iter()
        .find(|entry| {
            entry.name == args.guest_binary.to_uppercase()
                || bytemuck::cast::<[u32; 8], [u8; 32]>(entry.image_id) == potential_guest_image_id
        })
        .expect("Unknown guest binary");

    // Execute or return image id
    let output_bytes = match &args.input {
        Some(input) => {
            let input = hex::decode(&input[2..]).expect("Failed to decode image id");
            let input = bincode::serialize(&input).expect("Failed to serialize data");

            if env::var("BONSAI_ENDPOINT").is_ok() {
                prove_remote(guest_entry, input).expect("Failed to run proof with bonsai")
            } else {
                prove_locally(guest_entry, input)
            }
        }
        None => Vec::from(bytemuck::cast::<[u32; 8], [u8; 32]>(guest_entry.image_id)),
    };
    let output = hex::encode(output_bytes);
    print!("{output}");
    io::stdout().flush().unwrap();
}

#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use alloy_primitives::U256;
use alloy_sol_types::SolValue;
use bitcoin_spv::{types::HeaderArray, validatespv::validate_header_chain};
use risc0_zkvm::guest::env;
risc0_zkvm::guest::entry!(main);

fn main() {
    let (height, headers): (u64, Vec<u8>) = env::read();
    let headers = HeaderArray::new(&headers).unwrap();
    validate_header_chain(&headers, true).unwrap();
    let raw_header = headers.index(headers.len() - 1);
    let hash = raw_header.digest().as_ref().clone();

    let ret = (U256::from(height), hash.to_vec()).abi_encode();

    env::commit(&ret);
}

#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use bitcoin_spv::{types::HeaderArray, validatespv::validate_header_chain};
use risc0_zkvm::guest::env;
risc0_zkvm::guest::entry!(main);

fn main() {
    let data: (u64, Vec<u8>) = env::read();
    let headers = HeaderArray::new(&data.1).unwrap();
    validate_header_chain(&headers, true).unwrap();
    let raw_header = headers.index(headers.len() - 1);
    let ret = (data.0, raw_header.digest().as_ref().clone());
    env::commit(&ret);
}

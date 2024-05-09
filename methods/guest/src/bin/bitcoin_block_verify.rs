#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use bitcoin_spv::{types::HeaderArray, validatespv::validate_header_chain};
use ethabi::{ethereum_types::U256, Token};
use risc0_zkvm::guest::env;
risc0_zkvm::guest::entry!(main);

fn main() {
    let data: (u64, Vec<u8>) = env::read();
    let headers = HeaderArray::new(&data.1).unwrap();
    validate_header_chain(&headers, true).unwrap();
    let raw_header = headers.index(headers.len() - 1);
    let hash = raw_header.digest().as_ref().clone();
    let ret = ethabi::encode(&[
        Token::Uint(U256::from(data.0)),
        Token::FixedBytes(hash.to_vec()),
    ]);
    env::commit(&ret);
}

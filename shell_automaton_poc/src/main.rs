extern crate derive_more;
use service::mio::MioService;
use state::{Config, GlobalState, ImmutableState};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};
use tezos_identity::Identity;
use tezos_messages::p2p::encoding::version::NetworkVersion;

mod action;
mod service;
mod state;
mod transaction;

fn main_loop() -> io::Result<()> {
    let pow_target = 26.0;

    println!("Generating pow...");

    let config = Config {
        identity: Identity::generate(pow_target).unwrap(),
        network: NetworkVersion::new("TEZOS_MAINNET".to_string(), 0, 1),
        pow_target,
        max_peer_threshold: 30,
        disable_mempool: false,
        private_node: false,
    };

    let mut state = GlobalState::new(ImmutableState { config });
    let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 9732);

    println!("Listening connections");
    // TODO: handle errors
    let mut mio_service: MioService = MioService::try_new(SocketAddr::V4(addr)).unwrap();
    println!("Running loop");
    loop {
        mio_service.make_progress(&mut state);
    }
}

fn main() {
    main_loop().unwrap();
}

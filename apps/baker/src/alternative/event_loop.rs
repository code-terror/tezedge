// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    convert::TryInto,
    sync::mpsc,
    time::{Duration, SystemTime},
};

use reqwest::Url;
use slog::Logger;

use crypto::{
    blake2b,
    hash::{BlockHash, BlockPayloadHash, ContractTz1Hash, NonceHash, Signature, ChainId},
};
use tb::Payload;
use tenderbake as tb;
use tezos_encoding::types::SizedBytes;
use tezos_messages::protocol::proto_012::operation::{
    InlinedEndorsement, InlinedEndorsementMempoolContents, InlinedEndorsementMempoolContentsEndorsementVariant, InlinedPreendorsementContents, InlinedPreendorsementVariant, InlinedPreendorsement,
};

use super::{
    block_payload::BlockPayload,
    client::{RpcClient, RpcError},
    event::{Event, OperationKind, ProtocolBlockHeader},
    slots_info::SlotsInfo,
    timer::Timer,
    CryptoService,
    guess_proof_of_work,
};

pub fn run(endpoint: Url, crypto: &CryptoService, log: &Logger) -> Result<(), RpcError> {
    let (tx, rx) = mpsc::channel();
    let client = RpcClient::new(endpoint, tx.clone());
    let timer = Timer::spawn(tx);

    let chain_id = client.get_chain_id()?;
    client.wait_bootstrapped()?;

    let constants = client.get_constants()?;

    slog::info!(
        log,
        "committee size: {}",
        constants.consensus_committee_size
    );
    slog::info!(log, "pow threshold: {}", constants.proof_of_work_threshold);
    let config = tb::Config {
        consensus_threshold: 2 * (constants.consensus_committee_size / 3) + 1,
        minimal_block_delay: Duration::from_secs(constants.minimal_block_delay.parse().unwrap()),
        delay_increment_per_round: Duration::from_secs(
            constants.delay_increment_per_round.parse().unwrap(),
        ),
    };
    let proof_of_work_threshold = u64::from_be_bytes(
        constants
            .proof_of_work_threshold
            .parse::<i64>()
            .unwrap()
            .to_be_bytes(),
    );

    client.monitor_heads(&chain_id)?;

    let ours = vec![crypto.public_key_hash().clone()];
    let mut slots_info = SlotsInfo::new(constants.consensus_committee_size, ours);
    let mut state = tb::Machine::<ContractTz1Hash, BlockPayload>::empty();

    for event in rx {
        let unix_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let now = tb::Timestamp { unix_epoch };
        let actions = match event {
            Err(err) => {
                slog::error!(log, "{err}");
                vec![]
            }
            Ok(Event::Block(block)) => {
                slog::info!(log, "new block {}:{}", block.level, block.round);
                slots_info.insert(&block);
                let timestamp = tb::Timestamp {
                    unix_epoch: Duration::from_secs(block.timestamp),
                };
                let round = block.round;
                let pred_hash = block.predecessor.0.as_slice().try_into().unwrap();
                let proposal = Box::new(tb::BlockInfo {
                    pred_hash,
                    hash: block.hash.0.as_slice().try_into().unwrap(),
                    block_id: tb::BlockId {
                        level: block.level,
                        round,
                        payload_hash: block.payload_hash.0.as_slice().try_into().unwrap(),
                        payload_round: block.payload_round,
                    },
                    timestamp,
                    transition: block.transition,
                    prequorum: block.operations.first().and_then(|ops| {
                        let v = ops
                            .iter()
                            .filter_map(|op| match op.kind()? {
                                OperationKind::Preendorsement(v) => Some((v, op.clone())),
                                _ => None,
                            })
                            .collect::<Vec<_>>();

                        let (first, _) = v.first()?;
                        let level = first.level;
                        Some(tb::Prequorum {
                            block_id: tb::BlockId {
                                level,
                                round: first.round,
                                payload_hash: first
                                    .block_payload_hash
                                    .0
                                    .as_slice()
                                    .try_into()
                                    .unwrap(),
                                payload_round: first.round,
                            },
                            votes: {
                                v.into_iter()
                                    .filter_map(|(v, op)| {
                                        Some((slots_info.validator(level, v.slot)?, op))
                                    })
                                    .collect()
                            },
                        })
                    }),
                    quorum: block.operations.first().and_then(|ops| {
                        Some(tb::Quorum {
                            votes: {
                                ops.iter()
                                    .filter_map(|op| match op.kind()? {
                                        OperationKind::Endorsement(v) => Some((
                                            slots_info.validator(v.level, v.slot)?,
                                            op.clone(),
                                        )),
                                        _ => None,
                                    })
                                    .collect()
                            },
                        })
                    }),
                    payload: block.operations.into_iter().flatten().fold(
                        BlockPayload::default(),
                        |mut p, op| {
                            p.update(op);
                            p
                        },
                    ),
                });
                if let Err(err) = client.monitor_operations() {
                    slog::error!(log, "{}", err);
                }
                let event = tb::Event::Proposal(proposal, now);
                state
                    .handle(&config, &slots_info, event)
                    .into_iter()
                    .collect::<Vec<_>>()
            }
            Ok(Event::Operations(ops)) => {
                let mut actions = vec![];
                for op in ops {
                    match op.kind() {
                        None => slog::error!(log, "unclassified operation {op:?}"),
                        Some(OperationKind::Preendorsement(content)) => {
                            if let Some(content) = slots_info.preendorsement(&content) {
                                let event = tb::Event::Preendorsement(content, op, now);
                                actions
                                    .extend(state.handle(&config, &slots_info, event).into_iter());
                            }
                        }
                        Some(OperationKind::Endorsement(content)) => {
                            if let Some(content) = slots_info.endorsement(&content) {
                                let event = tb::Event::Endorsement(content, op, now);
                                actions
                                    .extend(state.handle(&config, &slots_info, event).into_iter());
                            }
                        }
                        Some(_) => {
                            state.handle(&config, &slots_info, tb::Event::PayloadItem(op));
                        }
                    }
                }
                actions
            }
            Ok(Event::Tick) => state
                .handle(&config, &slots_info, tb::Event::Timeout)
                .into_iter()
                .collect(),
        };
        perform(&client, log, &timer, crypto, &slots_info, &chain_id, constants.blocks_per_commitment, proof_of_work_threshold, actions);
    }

    Ok(())
}

fn perform(
    client: &RpcClient,
    log: &Logger,
    timer: &Timer,
    crypto: &CryptoService,
    slots_info: &SlotsInfo,
    chain_id: &ChainId,
    blocks_per_commitment: u32,
    proof_of_work_threshold: u64,
    actions: impl IntoIterator<Item = tb::Action<ContractTz1Hash, BlockPayload>>,
) {
    for action in actions {
        match action {
            tb::Action::ScheduleTimeout(timestamp) => {
                timer.schedule(timestamp);
            }
            tb::Action::Preendorse {
                pred_hash,
                block_id,
            } => {
                let this = crypto.public_key_hash();
                let slot = slots_info.slots(&this, block_id.level).and_then(|v| v.first());
                let slot = match slot {
                    Some(s) => *s,
                    None => continue,
                };
                let preendorsement = InlinedPreendorsement {
                    branch: BlockHash(pred_hash.to_vec()),
                    operations: InlinedPreendorsementContents::Preendorsement(
                        InlinedPreendorsementVariant {
                            slot,
                            level: block_id.level,
                            round: block_id.round,
                            block_payload_hash: BlockPayloadHash(
                                block_id.payload_hash.to_vec(),
                            ),
                        },
                    ),
                    signature: Signature(vec![]),
                };
                let (data, _) = crypto.sign(0x12, &chain_id, &preendorsement).unwrap();
                match client.inject_operation(&chain_id, hex::encode(data)) {
                    Ok(hash) => slog::info!(log, "injection/operation: {hash}"),
                    Err(err) => slog::error!(log, "{err}"),
                }
            }
            tb::Action::Endorse {
                pred_hash,
                block_id,
            } => {
                let this = crypto.public_key_hash();
                let slot = slots_info.slots(&this, block_id.level).and_then(|v| v.first());
                let slot = match slot {
                    Some(s) => *s,
                    None => continue,
                };
                let endorsement = InlinedEndorsement {
                    branch: BlockHash(pred_hash.to_vec()),
                    operations: InlinedEndorsementMempoolContents::Endorsement(
                        InlinedEndorsementMempoolContentsEndorsementVariant {
                            slot,
                            level: block_id.level,
                            round: block_id.round,
                            block_payload_hash: BlockPayloadHash(
                                block_id.payload_hash.to_vec(),
                            ),
                        },
                    ),
                    signature: Signature(vec![]),
                };
                let (data, _) = crypto.sign(0x13, &chain_id, &endorsement).unwrap();
                match client.inject_operation(&chain_id, hex::encode(data)) {
                    Ok(hash) => slog::info!(log, "injection/operation: {hash}"),
                    Err(err) => slog::error!(log, "{err}"),
                }
            }
            tb::Action::Propose(block, proposer) => {
                // TODO: multiple bakers
                let _ = proposer;
                let predecessor_hash = BlockHash(block.pred_hash.to_vec());
                let payload_round = block.block_id.payload_round;
                let payload_hash = if block.block_id.payload_hash == [0; 32] {
                    let operation_list_hash = block.payload.operation_list_hash().unwrap();
                    BlockPayloadHash::calculate(
                        &predecessor_hash,
                        payload_round as u32,
                        &operation_list_hash,
                    )
                    .unwrap()
                } else {
                    BlockPayloadHash(block.block_id.payload_hash.to_vec())
                };
                let pos_in_cycle =
                    (block.block_id.level as u32) % blocks_per_commitment;
                let mut protocol_header = ProtocolBlockHeader {
                    payload_hash,
                    payload_round,
                    seed_nonce_hash: if pos_in_cycle == 0 {
                        Some(NonceHash(blake2b::digest_256(&[1, 2, 3]).expect("constant")))
                    } else {
                        None
                    },
                    proof_of_work_nonce: SizedBytes(hex::decode("7985fafe1fb70300").unwrap().try_into().unwrap()),
                    liquidity_baking_escape_vote: false,
                    signature: Signature(vec![]),
                };
                let (_, signature) = crypto.sign(0x11, &chain_id, &protocol_header).unwrap();
                protocol_header.signature = signature;
                let timestamp = block.timestamp.unix_epoch.as_secs() as i64;

                let endorsements = block
                    .quorum
                    .map(|q| q.votes.ids.into_values())
                    .into_iter()
                    .flatten();
                let preendorsements = block
                    .prequorum
                    .map(|q| q.votes.ids.into_values())
                    .into_iter()
                    .flatten();
                let operations = [
                    endorsements.chain(preendorsements).collect::<Vec<_>>(),
                    block.payload.votes_payload,
                    block.payload.anonymous_payload,
                    block.payload.managers_payload,
                ];

                let (mut header, operations) = match client.preapply_block(
                    protocol_header,
                    predecessor_hash,
                    timestamp,
                    operations,
                ) {
                    Ok(v) => v,
                    Err(err) => {
                        slog::error!(log, "{err}");
                        continue;
                    }
                };

                header.signature.0 = vec![0x00; 64];
                let p = guess_proof_of_work(&header, proof_of_work_threshold);
                header.proof_of_work_nonce = SizedBytes(p);
                slog::info!(log, "{:?}", header);
                header.signature.0.clear();
                let (data, _) = crypto.sign(0x11, &chain_id, &header).unwrap();
                match client.inject_block(hex::encode(data), operations) {
                    Ok(hash) => slog::info!(log, "injection/block: {}, {hash}", header.level),
                    Err(err) => slog::error!(log, "{err}"),
                }
            }
        }
    }
}
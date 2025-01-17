// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

use std::path::PathBuf;

use crypto::hash::{ChainId, ContextHash, ProtocolHash};
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use strum_macros::IntoStaticStr;
// TODO: move some (or all) of these to this crate?
use tezos_api::ffi::{
    ApplyBlockError, ApplyBlockRequest, ApplyBlockResponse, BeginApplicationError,
    BeginApplicationRequest, BeginApplicationResponse, BeginConstructionError,
    BeginConstructionRequest, CommitGenesisResult, ComputePathError, ComputePathRequest,
    ComputePathResponse, DumpContextError, FfiJsonEncoderError, GetDataError,
    GetLastContextHashesError, HelpersPreapplyBlockRequest, HelpersPreapplyError,
    HelpersPreapplyResponse, InitProtocolContextResult, PrevalidatorWrapper, ProtocolDataError,
    ProtocolRpcError, ProtocolRpcRequest, ProtocolRpcResponse, RestoreContextError, RustBytes,
    TezosRuntimeConfiguration, TezosStorageInitError, ValidateOperationError,
    ValidateOperationRequest, ValidateOperationResponse,
};
use tezos_context_api::{
    ContextKeyOwned, ContextValue, GenesisChain, PatchContext, ProtocolOverrides, StringTreeObject,
    TezosContextStorageConfiguration,
};
use tezos_messages::p2p::encoding::operation::Operation;

/// This command message is generated by tezedge node and is received by the protocol runner.
#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
pub enum ProtocolMessage {
    ApplyBlockCall(ApplyBlockRequest),
    AssertEncodingForProtocolDataCall(ProtocolHash, RustBytes),
    BeginApplicationCall(BeginApplicationRequest),
    BeginConstructionForPrevalidationCall(BeginConstructionRequest),
    ValidateOperationForPrevalidationCall(ValidateOperationRequest),
    BeginConstructionForMempoolCall(BeginConstructionRequest),
    ValidateOperationForMempoolCall(ValidateOperationRequest),
    ProtocolRpcCall(ProtocolRpcRequest),
    HelpersPreapplyOperationsCall(ProtocolRpcRequest),
    HelpersPreapplyBlockCall(HelpersPreapplyBlockRequest),
    ComputePathCall(ComputePathRequest),
    ChangeRuntimeConfigurationCall(TezosRuntimeConfiguration),
    InitProtocolContextCall(InitProtocolContextParams),
    InitProtocolContextIpcServer(TezosContextStorageConfiguration),
    GenesisResultDataCall(GenesisResultDataParams),
    JsonEncodeApplyBlockResultMetadata(JsonEncodeApplyBlockResultMetadataParams),
    JsonEncodeApplyBlockOperationsMetadata(JsonEncodeApplyBlockOperationsMetadataParams),
    ContextGetKeyFromHistory(ContextGetKeyFromHistoryRequest),
    ContextGetKeyValuesByPrefix(ContextGetKeyValuesByPrefixRequest),
    ContextGetTreeByPrefix(ContextGetTreeByPrefixRequest),
    DumpContext(DumpContextRequest),
    RestoreContext(RestoreContextRequest),
    ContextGetLatestContextHashes(i64),
    Ping,
    ShutdownCall,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextGetKeyFromHistoryRequest {
    pub context_hash: ContextHash,
    pub key: ContextKeyOwned,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextGetKeyValuesByPrefixRequest {
    pub context_hash: ContextHash,
    pub prefix: ContextKeyOwned,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContextGetTreeByPrefixRequest {
    pub context_hash: ContextHash,
    pub prefix: ContextKeyOwned,
    pub depth: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DumpContextRequest {
    pub context_hash: ContextHash,
    pub dump_into_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RestoreContextRequest {
    pub expected_context_hash: ContextHash,
    pub restore_from_path: String,
    pub nb_context_elements: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InitProtocolContextParams {
    pub storage: TezosContextStorageConfiguration,
    pub genesis: GenesisChain,
    pub genesis_max_operations_ttl: u16,
    pub protocol_overrides: ProtocolOverrides,
    pub commit_genesis: bool,
    pub enable_testchain: bool,
    pub readonly: bool,
    pub patch_context: Option<PatchContext>,
    pub context_stats_db_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisResultDataParams {
    pub genesis_context_hash: ContextHash,
    pub chain_id: ChainId,
    pub genesis_protocol_hash: ProtocolHash,
    pub genesis_max_operations_ttl: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonEncodeApplyBlockResultMetadataParams {
    pub context_hash: ContextHash,
    pub metadata_bytes: RustBytes,
    pub max_operations_ttl: i32,
    pub protocol_hash: ProtocolHash,
    pub next_protocol_hash: ProtocolHash,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonEncodeApplyBlockOperationsMetadataParams {
    pub chain_id: ChainId,
    pub operations: Vec<Vec<Operation>>,
    pub operations_metadata_bytes: Vec<Vec<RustBytes>>,
    pub protocol_hash: ProtocolHash,
    pub next_protocol_hash: ProtocolHash,
}

/// This event message is generated as a response to the `ProtocolMessage` command.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(
    NodeMessageKind,
    derive(Serialize, Deserialize),
    cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))
)]
pub enum NodeMessage {
    ApplyBlockResult(Result<ApplyBlockResponse, ApplyBlockError>),
    AssertEncodingForProtocolDataResult(Result<(), ProtocolDataError>),
    BeginApplicationResult(Result<BeginApplicationResponse, BeginApplicationError>),
    BeginConstructionResult(Result<PrevalidatorWrapper, BeginConstructionError>),
    ValidateOperationResponse(Result<ValidateOperationResponse, ValidateOperationError>),
    RpcResponse(Result<ProtocolRpcResponse, ProtocolRpcError>),
    HelpersPreapplyResponse(Result<HelpersPreapplyResponse, HelpersPreapplyError>),
    ChangeRuntimeConfigurationResult,
    InitProtocolContextResult(Result<InitProtocolContextResult, TezosStorageInitError>),
    InitProtocolContextIpcServerResult(Result<(), String>), // TODO - TE-261: use actual error result
    CommitGenesisResultData(Result<CommitGenesisResult, GetDataError>),
    ComputePathResponse(Result<ComputePathResponse, ComputePathError>),
    JsonEncodeApplyBlockResultMetadataResponse(Result<String, FfiJsonEncoderError>),
    JsonEncodeApplyBlockOperationsMetadata(Result<String, FfiJsonEncoderError>),
    ContextGetKeyFromHistoryResult(Result<Option<ContextValue>, String>),
    ContextGetKeyValuesByPrefixResult(Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, String>),
    ContextGetTreeByPrefixResult(Result<StringTreeObject, String>),
    DumpContextResponse(Result<i64, DumpContextError>),
    RestoreContextResponse(Result<(), RestoreContextError>),
    ContextGetLatestContextHashesResult(Result<Vec<ContextHash>, GetLastContextHashesError>),

    // TODO: generic error response instead with error types?
    IpcResponseEncodingFailure(String),

    PingResult,

    ShutdownResult,
}

/// Empty message
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NoopMessage;

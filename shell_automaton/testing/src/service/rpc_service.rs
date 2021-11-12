// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use shell_automaton::service::rpc_service::{RpcResponse, RpcService, RpcRecvError, RpcId};

#[derive(Debug, Clone)]
pub struct RpcServiceDummy {}

impl RpcServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl RpcService for RpcServiceDummy {
    fn try_recv(&mut self) -> Result<(RpcResponse, RpcId), RpcRecvError> {
        Err(RpcRecvError::Empty)
    }

    fn respond(&mut self, call_id: RpcId, json: serde_json::Value) {
        let _ = (call_id, json);
    }
}
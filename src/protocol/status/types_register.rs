use std::sync::{Arc, Mutex};

use crate::protocol::status::{EntryRole, RegisterMode};

#[derive(Debug, Clone)]
pub struct RegisterEntry {
    pub role: EntryRole,
    pub slave_id: u8,
    pub mode: RegisterMode,
    pub address: u16,
    pub length: u16,
    pub values: Vec<u16>,
    pub next_poll_at: std::time::Instant,
    pub req_success: u32,
    pub req_total: u32,
    pub pending_requests: Vec<PendingRequest>,
}
impl Default for RegisterEntry {
    fn default() -> Self {
        Self {
            role: EntryRole::Slave,
            slave_id: 1,
            mode: RegisterMode::Holding,
            address: 0,
            length: 1,
            values: vec![0],
            next_poll_at: std::time::Instant::now(),
            req_success: 0,
            req_total: 0,
            pending_requests: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub func: u8,
    pub address: u16,
    pub count: u16,
    pub sent_at: std::time::Instant,
    pub request: Arc<Mutex<rmodbus::client::ModbusRequest>>,
}
impl PendingRequest {
    pub fn new(
        func: u8,
        address: u16,
        count: u16,
        sent_at: std::time::Instant,
        request: rmodbus::client::ModbusRequest,
    ) -> Self {
        Self {
            func,
            address,
            count,
            sent_at,
            request: Arc::new(Mutex::new(request)),
        }
    }
}

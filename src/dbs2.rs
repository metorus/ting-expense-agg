// #[sides(server)]

use tokio::sync::{broadcast, Mutex, RwLock};
use std::collections::HashMap;
use anyhow::{bail, Result};
use rusqlite::Connection;
use uuid::Uuid;

use crate::crosstyping::{UpstreamMessage, Expense, ClientData, Metadata};


pub struct MultiuserDb {
    conn: Mutex<Connection>,
    clients_notify_updates: RwLock<HashMap<String, broadcast::Sender<UpstreamMessage>>>
}

impl MultiuserDb {
    pub fn mem_new() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        
        conn.execute_batch("
BEGIN TRANSACTION;
CREATE TABLE spending_records (
    id        BLOB PRIMARY KEY  DEFAULT(randomblob(16)),
    principal TEXT              DEFAULT NULL,
    unix_date TEXT              DEFAULT(datetime('now')),
    amount_indivisible INT8,
    spend_group        TEXT,
    revoked            BOOL     DEFAULT FALSE
);
CREATE INDEX live_records ON spending_records(principal ASC, revoked ASC, unix_date DESC);
CREATE INDEX all_records ON spending_records(principal, unix_date DESC);
COMMIT;
        ").unwrap();
        
        Self {
            conn: Mutex::new(conn),
            clients_notify_updates: Default::default()
        }
    }
    
    pub async fn submit_expense(&self, principal: &str, d: ClientData, temp_alias: Uuid) -> Result<Expense> {
        if d.revoked {
            bail!("submitted expense couldn't be revoked already, before it got ID");
        }
        
        let expense = self.conn.lock().await.query_row("
INSERT INTO spending_records(amount_indivisible, spend_group, principal) VALUES(?1, ?2, ?3)
    RETURNING id,
              principal,
              unix_date;
        ", (d.amount, d.group.clone(), principal), |row| {
            let server = Metadata {
                uid:       row.get(0)?,
                principal: row.get(1)?,
                time:      row.get(2)?,
            };
            Ok(Expense{server, client: d})
        })?;
        
        // if there are WebSockets or SSEs connected, we must notify them
        if let Some(s) = self.clients_notify_updates.read().await.get(principal) {
            let _ = s.send(UpstreamMessage::NewSpending {
                expense: expense.clone(), temp_alias
            });
        }
        
        Ok(expense)
    }
    
    pub async fn submit_revoke(&self, principal: &str, total_id: Uuid) -> Result<Expense> {
        let expense = self.conn.lock().await.query_row("
UPDATE spending_records SET revoked = TRUE WHERE principal = ?1 AND id = ?2
    RETURNING id,
              principal,
              unix_date,
              amount_indivisible,
              spend_group,
              revoked;
        ", (principal, total_id), |row| {
            let server = Metadata {
                uid:       row.get(0)?,
                principal: row.get(1)?,
                time:      row.get(2)?,
            };
            let client = ClientData {
                amount:    row.get(3)?,
                group:     row.get(4)?,
                revoked:   row.get(5)?
            };
            assert!(client.revoked);
            Ok(Expense{server, client})
        })?;
        
        // if there are WebSockets or SSEs connected, we must notify them
        if let Some(s) = self.clients_notify_updates.read().await.get(principal) {
            let _ = s.send(UpstreamMessage::Revoked {
                expense: expense.clone()
            });
        }
        
        Ok(expense)
    }
    
    pub async fn subscribe(&self, principal: String) -> broadcast::Receiver<UpstreamMessage> {
        let mut clients = self.clients_notify_updates.write().await;
        clients
            .entry(principal)
            .or_insert_with(|| broadcast::channel(16).0)
            .subscribe()
    }
}


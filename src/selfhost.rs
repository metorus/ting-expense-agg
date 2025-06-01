// #[sides(client#selfhost)]

use std::collections::BTreeMap;
use rusqlite::Connection;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::crosstyping::*;


pub struct SingleUserSqlite {
    conn: Connection,
    report_stored_expenses: Vec<ClientboundUpdate>,
}
impl SingleUserSqlite {
    fn submit_expense(&mut self, d: ClientData, temp_alias: Uuid)  {
        let expense = self.conn.query_row("
INSERT INTO spending_records(amount_indivisible, spend_group) VALUES(?1, ?2)
   RETURNING id,
             principal,
             unix_date;
        ", (d.amount, d.group.clone()), |row| {
            // dbg!(row);
            
            let server = Metadata {
                uid:       row.get(0)?,
                principal: row.get(1)?,
                time:      row.get(2)?,
            };
            Ok(Expense{server, client: d})
        }).unwrap();
        
        self.report_stored_expenses.push(ClientboundUpdate::NewSpending{
            expense, temp_alias
        });
    }
    
    fn submit_revoke(&mut self, total_id: Uuid) {
        let _ = total_id;
        todo!()
    }
}

impl Default for SingleUserSqlite {
    fn default() -> Self {
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
CREATE INDEX live_records ON spending_records(principal, revoked, unix_date);
CREATE INDEX aggregate_records ON spending_records(principal, revoked, spend_group, unix_date);
COMMIT;
        ").unwrap();
        
        Self {conn, report_stored_expenses: Vec::with_capacity(1)}
    }
}

impl Upstream for SingleUserSqlite {
    fn submit(&mut self, d: ServerboundUpdate) {
        match d {
            ServerboundUpdate::Revoked{expense_id} => {
                self.submit_revoke(expense_id)
            },
            ServerboundUpdate::MadeExpense{info, temp_alias} => {
                self.submit_expense(info, temp_alias);
            },
            ServerboundUpdate::QueryHistory{..} => {},
        }
    }
    
    fn sync(&mut self) -> Vec<ClientboundUpdate> {
        self.report_stored_expenses.split_off(0)
    }
    
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)> {
        Some(Default::default())
    }
}


// ---------------------------------------------------------------------------------------------- //
// Test version which does not require building foreign code.

pub struct PseudoUpstream {
    uncommitted_expenses: Vec<(ClientData, Uuid)>,
    uncommitted_revokes: Vec<Uuid>,
    buffer_expenses: BTreeMap<Uuid, Expense>,
}
impl Default for PseudoUpstream {
    fn default() -> Self {
        Self {
            uncommitted_expenses: Vec::with_capacity(1),
            uncommitted_revokes: vec![],
            buffer_expenses: BTreeMap::new(),
        }
    }
}
impl Upstream for PseudoUpstream {
    fn submit(&mut self, d: ServerboundUpdate) {
        match d {
            ServerboundUpdate::Revoked{expense_id} => {
                self.uncommitted_revokes.push(expense_id);
            },
            ServerboundUpdate::MadeExpense{info, temp_alias} => {
                self.uncommitted_expenses.push((info, temp_alias));
            },
            ServerboundUpdate::QueryHistory{..} => {},
        }
    }
    fn sync(&mut self) -> Vec<ClientboundUpdate> {
        let mut v = Vec::with_capacity(self.uncommitted_expenses.len() +
                                       self.uncommitted_revokes.len());
        for (client, temp_alias) in self.uncommitted_expenses.drain(..) {
            let server = Metadata {
                uid: Uuid::new_v4(),
                time: OffsetDateTime::now_local().unwrap(),
                principal: None
            };
            let uid = server.uid.clone();
            let expense = Expense{server, client};
            self.buffer_expenses.insert(uid, expense.clone());
            v.push(ClientboundUpdate::NewSpending{expense, temp_alias});
        }
        v.extend(self.uncommitted_revokes.drain(..)
                     .flat_map(|i| -> Option<_> {
                         Some(ClientboundUpdate::Revoked{expense: self.buffer_expenses.remove(&i)?})
                     }));
        v
    }
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)> {
        Some(Default::default())
    }
}


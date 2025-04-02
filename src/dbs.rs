// #[sides(server,client)]

use rusqlite::Connection;
use uuid::Uuid;

use crate::crosstyping::{UpstreamMessage, Upstream, Expense, ClientData, 
                         DownstreamMessage, Metadata, CachedStats};


pub struct SingleUserSqlite {
    conn: Connection,
    report_stored_expenses: Vec<UpstreamMessage>,
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
        
        self.report_stored_expenses.push(UpstreamMessage::NewSpending{
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
CREATE INDEX live_records ON spending_records(
    principal ASC, revoked ASC, unix_date DESC);
CREATE INDEX all_records ON spending_records(principal, unix_date DESC);
COMMIT;
        ").unwrap();
        
        Self {conn, report_stored_expenses: Vec::with_capacity(1)}
    }
}

impl Upstream for SingleUserSqlite {
    fn submit(&mut self, d: DownstreamMessage) {
        match d {
            DownstreamMessage::Revoked{expense_id} => {
                self.submit_revoke(expense_id)
            },
            DownstreamMessage::MadeExpense{info, temp_alias} => {
                self.submit_expense(info, temp_alias);
            }
        }
    }
    
    fn sync(&mut self) -> Vec<UpstreamMessage> {
        self.report_stored_expenses.split_off(0)
    }
    
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)> {
        Some(Default::default())
    }
}


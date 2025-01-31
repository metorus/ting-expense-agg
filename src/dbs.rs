// #[sides(server,client)]

use rusqlite::Connection;

use crate::crosstyping::{UpstreamMessage, Upstream, Expense, ClientData, 
                         Metadata};


pub struct SingleUserSqlite {
    conn: Connection,
    report_stored_expenses: Vec<UpstreamMessage>,
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
    fn submit_expense(&mut self, d: ClientData<'static>, provisional_id: usize) {
        let (expense, true_id) = self.conn.query_row("
INSERT INTO spending_records(amount_indivisible, spend_group) VALUES(?1, ?2)
   RETURNING id,
             principal,
             unix_date,
             (SELECT count(*) FROM spending_records WHERE principal ISNULL);
        ", (d.amount, d.group.clone()), |row| {
            // dbg!(row);
            
            let server = Metadata {
                uid:       row.get(0)?,
                principal: row.get(1)?,
                time:      row.get(2)?,
            };
            let true_id_p1: usize = row.get(3)?;
            Ok((Expense{server, client: d}, true_id_p1 - 1))
        }).unwrap();
        
        let asked_id = Some(provisional_id);
        
        self.report_stored_expenses.push(UpstreamMessage::NewSpending{
            expense, asked_id, true_id
        });
    }
    
    fn submit_revoke(&mut self, _total_id: usize) {
        todo!()
    }
    
    fn sync(&mut self) -> Vec<UpstreamMessage> {
        self.report_stored_expenses.split_off(0)
    }
}


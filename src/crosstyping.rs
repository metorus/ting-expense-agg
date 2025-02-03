// #[sides(client, server)]

use time::{Duration,OffsetDateTime,format_description::well_known::Rfc3339};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::collections::BTreeMap;


pub const MONTH_LIKE: Duration = Duration::days(30);

//----------------------------------------------------------------------------//
/// Server-generated information about a certain expense.
/// If we intend to allow sharing information, these fields must not be
/// client-controllable at risk of falsification.
#[derive(Clone, Deserialize, Serialize)]
pub struct Metadata {
    pub uid: Uuid,
    pub time: OffsetDateTime,
    pub principal: Option<String>    // None stands for local
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ClientData {
    pub amount: u64,
    pub group: Option<String>,
    pub revoked: bool,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Expense {
    pub server: Metadata,
    pub client: ClientData
}

impl std::fmt::Display for Expense {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.client.revoked {
            return Err(std::fmt::Error);
        }
        write!(f, "{:08X} - {} - {}P on {}",
            self.server.uid.as_fields().0,
            self.server.time.format(&Rfc3339).unwrap(),
            self.client.amount,
            self.client.group.as_deref().unwrap_or("something")
        )
    }
}

#[derive(Default)]
pub struct CachedStats {
    pub records_alive: usize,
    pub group_spendings: Vec<(String, u64)>,
        group_indices: BTreeMap<String, usize>,
    pub total_spending: u64,
}
impl CachedStats {
    pub fn raw_add(&mut self, category: &str, amount: i64, d: isize) {
        let group_idx = match self.group_indices.get(category) {
            Some(i) => *i,
            None => {
                let i = self.group_spendings.len();
                self.group_spendings.push((category.to_owned(), 0));
                self.group_indices.insert(category.to_owned(), i);
                i
            }
        };
        self.group_spendings[group_idx].1 =
            self.group_spendings[group_idx].1.saturating_add_signed(amount);
        self.records_alive = self.records_alive.saturating_add_signed(d);
        self.total_spending =
            self.total_spending.saturating_add_signed(amount);
    }
}

//----------------------------------------------------------------------------//

#[derive(Clone, Deserialize, Serialize)]
pub enum UpstreamMessage {
    Revoked {expense: Expense},
    NewSpending {expense: Expense, temp_alias: Uuid},
}
#[derive(Clone, Deserialize, Serialize)]
pub enum DownstreamMessage {
    Revoked {expense_id: Uuid},
    MadeExpense {info: ClientData, temp_alias: Uuid}
}


pub trait Upstream {
    fn submit(&mut self, d: DownstreamMessage);
    fn sync(&mut self) -> Vec<UpstreamMessage>;
    
    /// Lifetime stats, month stats, at least month's worth of RECENTMOST
    /// confirmed expenses.
    fn take_init(&mut self) -> (CachedStats, CachedStats, Vec<Expense>);
}


pub struct PseudoUpstream {
    uncommitted_expenses: Vec<(ClientData, Uuid)>,
    uncommitted_revokes: Vec<Uuid>,
}
impl Default for PseudoUpstream {
    fn default() -> Self {
        Self {
            uncommitted_expenses: Vec::with_capacity(1),
            uncommitted_revokes: vec![],
        }
    }
}
impl Upstream for PseudoUpstream {
    fn submit(&mut self, d: DownstreamMessage) {
        match d {
            DownstreamMessage::Revoked{expense_id} => {
                self.uncommitted_revokes.push(expense_id);
            },
            DownstreamMessage::MadeExpense{info, temp_alias} => {
                self.uncommitted_expenses.push((info, temp_alias));
            }
        }
    }
    fn sync(&mut self) -> Vec<UpstreamMessage> {
        let mut v = Vec::with_capacity(self.uncommitted_expenses.len() +
                                       self.uncommitted_revokes.len());
        for (client, temp_alias) in self.uncommitted_expenses.drain(..) {
            let server = Metadata {
                uid: Uuid::new_v4(),
                time: OffsetDateTime::now_local().unwrap(),
                principal: None
            };
            let expense = Expense{server, client};
            v.push(UpstreamMessage::NewSpending{expense, temp_alias});
        }
        /*
        v.extend(self.uncommitted_revokes.drain(..)
                     .map(|i| UpstreamMessage::Revoked{expense_id: i}));
        */
        v
    }
    fn take_init(&mut self) -> (CachedStats, CachedStats, Vec<Expense>) {
        Default::default()
    }
}



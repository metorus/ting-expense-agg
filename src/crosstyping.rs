// #[sides(client, server)]

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
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

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CachedStats {
    pub records_alive: usize,
    pub group_spendings: Vec<(String, u64)>,
    #[serde(skip)] group_indices: BTreeMap<String, usize>,
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
    pub fn new(records: (u64, usize), group_spendings: Vec<(String, u64)>) -> Self {
        let (total_spending, records_alive) = records;
        let group_indices = BTreeMap::new();
        let mut this = Self {records_alive, group_spendings, group_indices, total_spending};
        this.set_indices();
        this
    }
    pub fn set_indices(&mut self) {
        self.group_indices.clear();
        for (i, (g, _)) in self.group_spendings.iter().enumerate() {
            self.group_indices.insert(g.to_owned(), i);
        }
    }
}

//----------------------------------------------------------------------------//

#[derive(Clone, Deserialize, Serialize)]
pub enum ClientboundUpdate {
    Revoked {expense: Expense},
    NewSpending {expense: Expense, temp_alias: Uuid},
    InitStats {lifetime_stats: CachedStats, recent_expenses: Vec<Expense>}
}
#[derive(Clone, Deserialize, Serialize)]
pub enum ServerboundUpdate {
    Revoked {expense_id: Uuid},
    MadeExpense {info: ClientData, temp_alias: Uuid},
}


pub trait Upstream {
    fn submit(&mut self, d: ServerboundUpdate);
    fn sync(&mut self) -> Vec<ClientboundUpdate>;
    
    /// Lifetime stats, month stats, at least month's worth of RECENTMOST
    /// confirmed expenses.
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)>;
}



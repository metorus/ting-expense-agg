// #[sides(client, server)]

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use serde::{Deserialize, Serialize};
use uuid::Uuid;


pub const UNCLASSIFIED: &str = "покупки";
pub const MONTH_LIKE: Duration = Duration::days(30);

//----------------------------------------------------------------------------//
/// Server-generated information about a certain expense.
/// If we intend to allow sharing information, these fields must not be
/// client-controllable at risk of falsification.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Metadata {
    pub uid: Uuid,
    pub time: OffsetDateTime,
    pub principal: Option<String>    // None stands for local
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClientData {
    pub amount: u64,
    pub group: Option<String>,
    pub revoked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Expense {
    pub server: Metadata,
    pub client: ClientData
}

impl std::fmt::Display for Expense {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.client.revoked {
            return Err(std::fmt::Error);
        }
        write!(f, "{:08X} - {} - {}\u{20bd} на {}",
            self.server.uid.as_fields().0,
            self.server.time.format(&Rfc3339).unwrap(),
            self.client.amount,
            self.client.group.as_deref().unwrap_or(UNCLASSIFIED)
        )
    }
}

#[cfg(feature = "graphics")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CachedStats {
    pub records_alive: usize,
    pub group_spendings: Vec<(String, u64)>,
    #[serde(skip)] group_indices: std::collections::BTreeMap<String, usize>,
    pub total_spending: u64,
}
#[cfg(feature = "graphics")]
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
    #[cfg_attr(
        feature = "graphics",
        warn(dead_code, reason = "Any +graphics combination must accept incoming expenses")
    )]
    pub fn add(&mut self, e: &Expense) {
        self.raw_add(e.client.group.as_deref().unwrap_or(UNCLASSIFIED), e.client.amount as i64, 1);
    }
    #[cfg_attr(
        feature = "graphics",
        warn(dead_code, reason = "Any +graphics combination must push month-old expenses out")
    )]
    pub fn sub(&mut self, e: &Expense) {
        let inv_amount = -(e.client.amount as i64);
        self.raw_add(e.client.group.as_deref().unwrap_or(UNCLASSIFIED), inv_amount, -1);
    }
    #[allow(dead_code, reason = "+selfhost does not need this as it creates default instances")]
    pub fn new(records: ((u64, usize), Vec<(String, u64)>)) -> Self {
        let ((total_spending, records_alive), group_spendings) = records;
        let group_indices = std::collections::BTreeMap::default();
        let mut this = Self {records_alive, group_spendings, group_indices, total_spending};
        this.set_indices();
        this
    }
    fn set_indices(&mut self) {
        self.group_indices.clear();
        for (i, (g, _)) in self.group_spendings.iter().enumerate() {
            self.group_indices.insert(g.to_owned(), i);
        }
    }
}

//----------------------------------------------------------------------------//

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ClientboundUpdate {
    Revoked {expense: Expense},
    NewSpending {expense: Expense, temp_alias: Uuid},
    InitStats {lifetime_stats: ((u64, usize), Vec<(String, u64)>), recent_expenses: Vec<Expense>}
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ServerboundUpdate {
    Revoked {expense_id: Uuid},
    MadeExpense {info: ClientData, temp_alias: Uuid},
}

#[cfg(feature = "graphics")]
pub trait Upstream {
    fn submit(&mut self, d: ServerboundUpdate);
    fn sync(&mut self) -> Vec<ClientboundUpdate>;
    
    /// Lifetime stats, month stats, at least month's worth of RECENTMOST
    /// confirmed expenses.
    fn take_init(&mut self) -> Option<(CachedStats, CachedStats, Vec<Expense>)>;
}



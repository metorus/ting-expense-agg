// targets: ALL

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use std::borrow::Cow;
use uuid::Uuid;


//----------------------------------------------------------------------------//
/// Monotonic time measurements in form useful for external entities.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LogicalTime {
    pub generation: u32,
    pub instant:    OffsetDateTime
}
impl LogicalTime {
    /// Returns a reference-zero time point.
    /// It might not be the smallest time representable by chosen type but all
    /// measurements will point past that moment.
    fn zero() -> Self {
        Self {generation: 0, instant: OffsetDateTime::UNIX_EPOCH}
    }
    
    /// Issues next measurement, incrementing generation number if wall clock
    /// happened to show an earlier time than previously.
    /// 
    /// Note this will not always detect clock rolling back because the method
    /// might not get used till the time passed once again; nevertheless, it is
    /// guaranteed that the returned timestamps increase monotonically.
    fn now(last_stamp: &mut LogicalTime) -> Self {
        // `last_stamp` is already initialized and `now()` returns a new value,
        //  so the new measurement is always AFTER the previous one. We can just
        //  increase the generation if needed.
        // Transitivity ensured by deriving `PartialOrd` / `Ord`.
        
        let last_instant = last_stamp.instant;
        
        let now = OffsetDateTime::now_local().unwrap()
                                 .replace_nanosecond(0).unwrap();
        let gen = last_stamp.generation + if now < last_instant {1} else {0};
        let obj = Self {generation: gen, instant: now};
        
        debug_assert!(obj >= *last_stamp);
        
        *last_stamp = obj;  obj
    }
}

pub type Interval = (LogicalTime, LogicalTime);

pub const MONTH_LIKE: Duration = Duration::days(30);

//----------------------------------------------------------------------------//
/// Server-generated information about a certain expense.
/// If we intend to allow sharing information, these fields must not be
/// client-controllable at risk of falsification.
#[derive(Clone)]
pub struct Metadata<'de> {
    pub uid: Uuid,
    pub time: LogicalTime,
    pub principal: Option<Cow<'de, str>>    // None stands for local
}

#[derive(Clone)]
pub struct ClientData<'de> {
    pub amount: u64,
    pub group: Option<Cow<'de, str>>,
    pub revoked: bool,
}

#[derive(Clone)]
pub struct Expense<'de> {
    pub server: Metadata<'de>,
    pub client: ClientData<'de>
}

impl<'de> std::fmt::Display for Expense<'de> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.client.revoked {
            return Err(std::fmt::Error);
        }
        write!(f, "{:08X} - [{}]{} - {}P on {}",
            self.server.uid.as_fields().0,
            self.server.time.generation,
            self.server.time.instant.format(&Rfc3339).unwrap(),
            self.client.amount,
            self.client.group.as_ref().map_or("something", |cow| cow.as_ref())
        )
    }
}

//----------------------------------------------------------------------------//

pub enum UpstreamMessage {
    Revoked{total_id: usize, asked_here: bool},
    NewSpending{expense: Expense<'static>, asked_id: Option<usize>,
                true_id: usize},
}

pub trait Upstream {
    fn submit_expense(&mut self, d: ClientData<'static>, provisional_id: usize);
    fn submit_revoke(&mut self, total_id: usize);
    fn sync(&mut self) -> Vec<UpstreamMessage>;
}


pub struct PseudoUpstream {
    uncommitted_expenses: Vec<(ClientData<'static>, usize)>,
    uncommitted_revokes: Vec<usize>,
    last_marker: LogicalTime,
}
impl Default for PseudoUpstream {
    fn default() -> Self {
        Self {
            uncommitted_expenses: Vec::with_capacity(1),
            uncommitted_revokes: vec![],
            last_marker: LogicalTime::zero(),
        }
    }
}
impl Upstream for PseudoUpstream {
    fn submit_expense(&mut self, d: ClientData<'static>, asked_id: usize) {
        self.uncommitted_expenses.push((d, asked_id));
    }
    fn submit_revoke(&mut self, total_id: usize) {
        self.uncommitted_revokes.push(total_id);
    }
    fn sync(&mut self) -> Vec<UpstreamMessage> {
        let mut v = Vec::with_capacity(self.uncommitted_expenses.len() +
                                   self.uncommitted_revokes.len());
        for (client, asked_id) in self.uncommitted_expenses.drain(..) {
            let server = Metadata {
                uid: Uuid::new_v4(),
                time: LogicalTime::now(&mut self.last_marker),
                principal: None
            };
            let expense = Expense{server, client};
            v.push(UpstreamMessage::NewSpending{
                expense,
                asked_id: Some(asked_id),
                true_id: asked_id
            });
        }
        v.extend(self.uncommitted_revokes.drain(..)
                     .map(|i| UpstreamMessage::Revoked{total_id: i,
                                                       asked_here: true}));
        v
    }
}



use std::time::{Duration, SystemTime};
use std::borrow::Cow;
use uuid::Uuid;


//----------------------------------------------------------------------------//
/// Monotonic time measurements in form useful for external entities.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LogicalTime {
    pub generation: u32,
    pub instant:    SystemTime
}
impl LogicalTime {
    /// Returns a reference-zero time point.
    fn zero() -> Self {
        Self {generation: 0, instant: SystemTime::UNIX_EPOCH}
    }
    
    /// Issues a next measurement, incrementing generation number if wall clock
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
        
        let now = SystemTime::now();
        let gen = last_stamp.generation
                + if now.duration_since(last_stamp).is_err() {1} else {0};
        let obj = Self {generation: gen, instant: now}
        
        debug_assert!(obj >= *last_stamp);
        
        *last_stamp = obj;  obj
    }
}

pub type Interval = (LogicalTime, LogicalTime);


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
    pub group: Option<Cow<'de, str>>
}

#[derive(Clone)]
pub struct Expense<'de> {
    pub server: Metadata<'de>,
    pub client: ClientData<'de>
}

fn to_key(e: &Expense<'_>) -> &LogicalTime {
    &e.server.time
}


//----------------------------------------------------------------------------//
// Our operations fundamentally don't include principals interaction. Therefore,
// we can model all of those as separate - and possibly locally stored.

pub struct LastInfo {
    pub total_amount: u64,
    pub count: usize,
    pub times: Interval,
    pub group: Option<String>,
}

pub trait TunedDb {
    fn gen_interval_last(&mut self, dur: Duration) -> Interval;
    fn gen_server_data(&mut self) -> Metadata<'static>;
    
    fn total_spending_last(&self, t: Interval, group: Option<&str>) -> LastInfo;
    
    fn insert_expense(&mut self, e: ClientData<'static>) -> Expense<'_>;
}



pub struct FallbackDb {
    last_marker: LogicalTime,
    operations:  Vec<Expense<'static>>
}
impl TunedDb for FallbackDb {
    fn gen_interval_last(&mut self, dur: Duration) -> Interval {
        let now = LogicalTime::now(&mut self.last_marker);
        
    }
    fn gen_server_data(&mut self) -> Metadata<'static> {
        Metadata {
            uid: Uuid::new_v4(),
            time: LogicalTime::now(&mut self.last_marker),
            principal: None
        }
    }
    fn total_spending_last(&self, t: Interval, group: Option<&str>) -> LastInfo {
        let b = self.operations.partition_point(|&op| to_key(op) <= t.1);
        let a = self.operations[..b].partition_point(|&op| to_key(op) < t.0);
        let mut u = 0;
        let mut c = 0;
        for op in &self.operations[a..b] {
            if let Some(g) = group {
                if op.client.group != Some(g.into()) {continue;}
            }
            c += 1;
            u += op.client.amount;
        }
        LastInfo {
            total_amount: u,
            count: c,
            times: t,
            group: group.map(String::to_owned)
        }
    }
    fn insert_expense(&mut self, c: ClientData<'static>) -> Expense<'_> {
        let mut s = self.gen_server_data();
        let e = Expense{server: s, client: c};
        self.operations.push(e.clone());
        e
    }
}



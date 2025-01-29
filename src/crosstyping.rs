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

fn to_key<'a>(e: &'a Expense<'_>) -> &'a LogicalTime {
    &e.server.time
}

fn principal_match(expense: &Option<Cow<'_, str>>, our: &Option<Cow<'_, str>>)
    -> bool { expense.is_none() || expense == our }


//----------------------------------------------------------------------------//
// Our operations fundamentally don't include principals interaction. Therefore,
// we can model all of those as separate - and possibly locally stored.

pub struct LastInfo<EntryRef: Clone> {
    pub total_amount: u64,
    pub count: usize,
    pub bound: (EntryRef, EntryRef),
}

pub trait TunedDb {
    type Er: Clone;
    
    fn gen_interval_last(&mut self, dur: Duration) -> Interval;
    fn gen_server_data(&mut self) -> Metadata<'static>;
    
    fn aggregate(&self, t: Interval, group: Option<&str>) -> LastInfo<Self::Er>;
    fn insert_expense(&mut self, e: ClientData<'static>) -> Self::Er;
    fn load(&self, entry_ref: Self::Er) -> &Expense<'_>;
    fn revoke(&mut self, i: usize);
}



pub struct FallbackDb {
    last_marker: LogicalTime,
    operations:  Vec<Expense<'static>>
}
impl Default for FallbackDb {
    fn default() -> Self {
        Self {last_marker: LogicalTime::zero(),
              operations: Vec::with_capacity(128)}
    }
}
impl TunedDb for FallbackDb {
    type Er = usize;
    
    fn gen_interval_last(&mut self, dur: Duration) -> Interval {
        //          |=================|
        //  W->-W---W---W---W
        //        ,________/
        //       W---W---W---W
        //             ,____/
        //            W---W---W---W---W-->
        //          |=================|
        // now, we only select the last branch
        
        let now = LogicalTime::now(&mut self.last_marker);
        let instant = now.instant.checked_sub(dur)
                                 .unwrap_or(OffsetDateTime::UNIX_EPOCH);
        // TODO: smarter algorithm to select `generation`
        (LogicalTime{generation: now.generation, instant}, now)
    }
    fn gen_server_data(&mut self) -> Metadata<'static> {
        Metadata {
            uid: Uuid::new_v4(),
            time: LogicalTime::now(&mut self.last_marker),
            principal: None
        }
    }
    fn aggregate(&self, t: Interval, group: Option<&str>) -> LastInfo<usize> {
        let b = self.operations.partition_point(|op| *to_key(&op) <= t.1);
        let a = self.operations[..b].partition_point(|op| *to_key(&op) < t.0);
        let mut u = 0;
        let mut c = 0;
        for op in &self.operations[a..b] {
            if op.client.revoked {continue;}
            if let Some(g) = group {
                if op.client.group != Some(g.into()) {continue;}
            }
            if !principal_match(&op.server.principal, &None) {continue;}
            c += 1;
            u += op.client.amount;
        }
        LastInfo {
            total_amount: u,
            count: c,
            bound: (a, b),
        }
    }
    fn insert_expense(&mut self, c: ClientData<'static>) -> Self::Er {
        let s = self.gen_server_data();
        let e = Expense{server: s, client: c};
        self.operations.push(e);
        self.operations.len() - 1
    }
    fn load(&self, i: usize) -> &Expense<'_> {
        &self.operations[i]
    }
    fn revoke(&mut self, i: usize) {
        self.operations[i].client.revoked = true;
    }
}



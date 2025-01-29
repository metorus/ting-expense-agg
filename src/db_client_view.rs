use time::OffsetDateTime;

use std::collections::{BTreeMap, VecDeque};
use std::ops::Range;

use crate::crosstyping::{ClientData, Expense, MONTH_LIKE};



#[derive(Default)]
pub struct CachedStats {
    pub records_range: (usize, usize),
    
    pub records_alive: usize,
    pub group_spendings: Vec<(String, u64)>,
        group_indices: BTreeMap<String, usize>,
    pub total_spending: u64,
}
impl CachedStats {
    pub fn push_back(&mut self, spent: (&str, u64, usize)) {
        let (category, amount, record_i) = spent;
        assert!(record_i >= self.records_range.1,
            "new spending record should be later than all known");
        self.records_range.1 = record_i + 1;
        
        let group_idx = match self.group_indices.get(category) {
            Some(i) => *i,
            None => {
                let i = self.group_spendings.len();
                self.group_spendings.push((category.to_owned(), 0));
                self.group_indices.insert(category.to_owned(), i);
                i
            }
        };
        self.group_spendings[group_idx].1 += amount;
        self.records_alive += 1;
        self.total_spending += amount;
    }
    
    fn range(&self) -> Range<usize> {
        self.records_range.0 .. self.records_range.1
    }
    
    pub fn revoke(&mut self, unspent: (&str, u64, usize)) {
        let (category, amount, record_i) = unspent;
        assert!(self.range().contains(&record_i),
            "revoked spending record should be in known range");
        
        let group_idx = *self.group_indices.get(category)
            .expect("revoked record's group should be known");
        self.group_spendings[group_idx].1 -= amount;
        self.records_alive -= 1;
        self.total_spending -= amount;
    }
    
    pub fn borrow(&self) -> &[(String, u64)] {
        &self.group_spendings
    }
    
    /*  this would not be a short-returning function
    pub fn trim_linear(&mut self, loader: FnMut(usize) -> (&str, u64, bool)) {
        while !self.range.is_empty() {
            let i = self.records_range.0;
            let (category, amount, expired) = loader(i);
            if !expired {break;}
            
            self.records_range.0 += 1;
            let group_idx = self.group_indices.get(category)
                .expect("trimmed record's group should be known");
            self.group_spendings[group_idx].1 -= amount;
            self.records_alive -= 1;
            self.total_spending -= amount;
        }
    }
    */
    
    pub fn trim_linear<'a>(&mut self,
            mut loader: impl FnMut(usize) -> Option<(&'a str, u64, bool)>) {
        while !self.range().is_empty() {
            let i = self.records_range.0;
            let Some((category, amount, expired)) = loader(i) else {break};
            if !expired {break;}
            
            self.records_range.0 += 1;
            let group_idx = *self.group_indices.get(category)
                .expect("trimmed record's group should be known");
            self.group_spendings[group_idx].1 -= amount;
            self.records_alive -= 1;
            self.total_spending -= amount;
        }
    }
}



type DeqView<T> = (usize, VecDeque<T>);
pub type MayLoad<'a> = Result<&'a Expense<'a>, ()>;

#[derive(Default)]
pub struct DbView<U> {
    upstream: U,
    pie_cache_month: CachedStats,
    pie_cache_forever: CachedStats,
    loaded_records: DeqView<Expense<'static>>,  // aims to have last month known
    
    // live_records.0 (first field of DeqView) is same as loaded_records.0
    //   and stands for "what's the index of oldest known spending record"
    //   (that is, "how many spending records can be loaded into the past").
    // live_records.1 is deque of (record number, data) pairs.
    live_records: DeqView<(usize, Expense<'static>)>,
}
impl<U> DbView<U> {
    pub fn month_transactions_info(&mut self) -> (u64, usize) {
        self.trim_caches();
        self.sync_upstream();
        
        let stats: &CachedStats = &self.pie_cache_month;
        (stats.total_spending, stats.records_alive)
    }
    
    pub fn month_pie(&mut self) -> &[(String, u64)] {
        self.trim_caches();
        self.sync_upstream();
        
        self.pie_cache_month.borrow()
    }
    
    pub fn load_last_spendings(&mut self, n: usize) ->
            impl Iterator<Item=MayLoad<'_>> {
        self.trim_caches();
        self.sync_upstream();
        
        self.live_records.1.iter().rev().map(|v| Ok(&v.1))
            .chain(std::iter::repeat(Err(())))
            .take(n)
    }
    
    pub fn insert_expense(&mut self, d: ClientData) {
        // We aren't gonna have ServerData like expense's index until upstream
        //   responds, but we must already show it.
        
        assert!(!d.revoked, "we shouldn't submit already-revoked records");
        
        // Generating ID assuming no one is writing in parallel.
        let provisional_id = self.pie_cache_forever.records_range.1;
        self.pie_cache_month.push_back((
            d.group.as_ref().map_or("unclassified", |cow| cow.as_ref()),
            d.amount,
            provisional_id
        ));
        self.pie_cache_forever.push_back((
            d.group.as_ref().map_or("unclassified", |cow| cow.as_ref()),
            d.amount,
            provisional_id
        ));
        
        // u.submit_expense((d, hint_now));
        // Adding to deque views will be handled by DbView::sync_upstream;
        //   there will be no need to revoke
    }
    
    fn trim_caches(&mut self) {
        let known = &self.loaded_records;
        let now = OffsetDateTime::now_local().unwrap()
                                 .replace_nanosecond(0).unwrap();
        let liveline = now - MONTH_LIKE;
        
        //          |=================|
        //  W->-W---W---W---W
        //        ,________/
        //       W---W---W---W
        //             ,____/
        //            W---W---W---W---W-->
        //          |=================|
        // 
        // in general, we attempt to keep first branch which measured time
        //   of at least (now - MONTH_LIKE) and everything after it
        // linear cache trimming means we can simply compare timestamps!
        //   (TODO: mathematical proof)
        
        self.pie_cache_month.trim_linear(|i| {
            let Some(i) = i.checked_sub(known.0) else {return None};
            let Some(expense) = known.1.get(i) else {return None};
            let group = expense.client.group
                .as_ref()
                .map_or("unclassified", |cow| cow.as_ref());
            let amount = expense.client.amount;
            let expired = expense.server.time.instant < liveline;
            Some((group, amount, expired))
        });
    }
    
    fn sync_upstream(&mut self) {
        // - handle  Revoked(i)  messages
        // - handle  NewSpending(expense,asked_id,true_id)  messages
        // - handle  NewSpendings(expense[])  messages
        // - periodically ping and validate last record ID
    }
}


use time::OffsetDateTime;

use std::collections::{BTreeMap, VecDeque};
use std::ops::Range;

use crate::crosstyping::{ClientData, Expense, MONTH_LIKE};
use crate::crosstyping::{Upstream, UpstreamMessage};



#[derive(Default)]
pub struct CachedStats {
    pub records_range: (usize, usize),
    
    pub records_alive: usize,
    pub group_spendings: Vec<(String, u64)>,
        group_indices: BTreeMap<String, usize>,
    pub total_spending: u64,
}
impl CachedStats {
    fn raw_add(&mut self, category: &str, amount: u64) {
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
    fn raw_sub(&mut self, category: &str, amount: u64) {
        let group_idx = *self.group_indices.get(category)
            .expect("revoked record's group should be known");
        self.group_spendings[group_idx].1 -= amount;
        self.records_alive -= 1;
        self.total_spending -= amount;
    }
    
    
    pub fn push_back(&mut self, spent: (&str, u64, usize)) {
        let (category, amount, record_i) = spent;
        assert!(record_i >= self.records_range.1,
            "new spending record should be later than all known");
        self.records_range.1 = record_i + 1;
        self.raw_add(category, amount);
    }
    
    fn range(&self) -> Range<usize> {
        self.records_range.0 .. self.records_range.1
    }
    
    pub fn user_revoke(&mut self, unspent: (&str, u64, usize)) {
        let (category, amount, record_i) = unspent;
        assert!(self.range().contains(&record_i),
            "revoked spending record should be in known range");
        self.raw_sub(category, amount);
    }
    
    pub fn rewrite(&mut self, record_i: usize,
            unspent: (&str, u64), spent: (&str, u64)) {
        if !self.range().contains(&record_i) {return;}
        //  "rewritten spending record should be in known range");
        self.raw_sub(unspent.0, unspent.1);
        self.raw_add(spent.0, spent.1);
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
pub type MayLoad<'a> = Result<(usize, &'a Expense<'a>), ()>;

fn group_amount<'a>(e: &'a Expense<'a>) -> (&'a str, u64) {
    (e.client.group.as_ref().map_or("unclassified", |cow| cow.as_ref()),
     e.client.amount)
}

#[derive(Default)]
pub struct DbView<U: Upstream> {
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
impl<U: Upstream> DbView<U> {
    pub fn with(upstream: U) -> Self {
        Self {
            upstream,
            pie_cache_month: CachedStats::default(),  // TODO: change
            pie_cache_forever: CachedStats::default(),  // TODO: change
            loaded_records: Default::default(),  // TODO: change
            live_records: Default::default(),  // TODO: change
        }
    }
    
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
        
        self.live_records.1.iter().rev().map(|v| Ok((v.0, &v.1)))
            .chain(std::iter::repeat_n(Err(()), self.live_records.0))
            .take(n)
    }
    
    pub fn insert_expense(&mut self, d: ClientData<'static>) {
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
        
        self.upstream.submit_expense(d, provisional_id);
        // Adding to deque views will be handled by DbView::sync_upstream;
        //   record will be rewritten if it's moved.
    }
    
    fn revoke_base(lr: &mut DeqView<Expense<'static>>,
                   pie_cache_month: &mut CachedStats,
                   pie_cache_forever: &mut CachedStats,
                   i: usize) {
        if i >= lr.0 {
            let expense = &lr.1[i - lr.0];
            let (group, amount) = group_amount(expense);
            if i >= pie_cache_month.records_range.0 {
                pie_cache_month.user_revoke((group, amount, i));
            }
            pie_cache_forever.user_revoke((group, amount, i));
        }
    }
        
    pub fn revoke(&mut self, i: usize, bypass_upstream: bool) {
        Self::revoke_base(&mut self.loaded_records, &mut self.pie_cache_month,
            &mut self.pie_cache_forever, i);
        if !bypass_upstream {
            self.upstream.submit_revoke(i);
        }
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
            let (group, amount) = group_amount(expense);
            let expired = expense.server.time.instant < liveline;
            Some((group, amount, expired))
        });
    }
    
    fn sync_upstream(&mut self) {
        // - handle  Revoked(i)  messages
        // - handle  NewSpending(expense,asked_id,true_id)  messages
        // - handle  NewSpendings(expense[])  messages
        // - periodically ping and validate last record ID
        
        for msg in self.upstream.sync() {
            match msg {
                UpstreamMessage::Revoked{total_id, asked_here} => {
                    if !asked_here {
                        // another client revoked an expense; we should
                        // remove it from stats, but not resending revocation
                        self.revoke(total_id, true);
                    }
                    
                    // remove from `live_records`
                    let lr = &mut self.live_records.1;
                    if let Ok(p) = lr.binary_search_by(|x| x.0.cmp(&total_id)) {
                        lr.remove(p);
                    }
                    
                    // update in `loaded_records`
                    let lr = &mut self.loaded_records;
                    if let Some(p) = total_id.checked_sub(lr.0) {
                        lr.1[p].client.revoked = true;
                    }
                },
                UpstreamMessage::NewSpending{expense, asked_id, true_id} => {
                    let known = asked_id.is_some();
                    
                    let i = true_id - self.loaded_records.0;
                    'mt: { match self.loaded_records.1.get_mut(i) {
                        None => {
                            let (group, amount) = group_amount(&expense);
                            self.loaded_records.1.push_back(expense.clone());
                            self.live_records.1.push_back((true_id,
                                expense.clone()));
                            
                            if known {break 'mt;}
                            self.pie_cache_month.push_back((group, amount,
                                true_id));
                            self.pie_cache_forever.push_back((group, amount,
                                true_id));
                        }
                        Some(expense_old) => {
                            // We have to rewrite something.
                            self.pie_cache_month.rewrite(true_id,
                                group_amount(expense_old),group_amount(&expense)
                            );
                            self.pie_cache_forever.rewrite(true_id,
                                group_amount(expense_old),group_amount(&expense)
                            );
                            
                            // Checking if `live_records` also needs this change
                            if let Ok(p) = self.live_records.1.binary_search_by(
                                    |x| x.0.cmp(&true_id)) {
                                
                                if expense.client.revoked {
                                    self.live_records.1.remove(p);
                                } else {
                                    self.live_records.1[p] =
                                        (true_id, expense.clone());
                                }
                            }
                            
                            *expense_old = expense;
                        }
                    }}
                },
            }
        }
    }
}


// #[sides(client)]

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
            .expect("subtracted record's group should be known");
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
    
    pub fn shift(&mut self, record_i: usize, record_to: usize,
            spent: (&str, u64)) {
        
        let was_included = self.range().contains(&record_i);
        let must_include = self.range().contains(&record_to);
        
        if !was_included && must_include {self.raw_add(spent.0, spent.1);}
        if was_included && !must_include {self.raw_sub(spent.0, spent.1);}
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
            self.raw_sub(category, amount);
        }
    }
}



type DeqView<T> = (usize, VecDeque<T>);
pub type MayLoad<'a> = Result<(usize, &'a Expense<'a>), ()>;

fn group_amount_c<'a>(e: &'a ClientData<'a>) -> (&'a str, u64) {
    (e.group.as_ref().map_or("unclassified", |cow| cow.as_ref()),
     e.amount)
}
fn group_amount<'a>(e: &'a Expense<'a>) -> (&'a str, u64) {
    group_amount_c(&e.client)
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
    
    provisional: BTreeMap<usize, ClientData<'static>>,
}
impl<U: Upstream> DbView<U> {
    pub fn with(upstream: U) -> Self {
        Self {
            upstream,
            pie_cache_month: CachedStats::default(),  // TODO: change
            pie_cache_forever: CachedStats::default(),  // TODO: change
            loaded_records: Default::default(),  // TODO: change
            live_records: Default::default(),  // TODO: change
            provisional: BTreeMap::new(),
        }
    }
    
    pub fn month_transactions_info(&mut self) -> (u64, usize) {
        self.trim_caches();
        self.sync_upstream();
        
        let stats: &CachedStats = &self.pie_cache_month;
        (stats.total_spending, stats.records_alive)
    }
    
    pub fn total_live_transactions(&mut self) -> usize {
        // self.trim_caches();
        // self.sync_upstream();
        
        self.pie_cache_forever.records_alive
    }
    
    pub fn month_pie(&mut self) -> &[(String, u64)] {
        self.trim_caches();
        self.sync_upstream();
        
        self.pie_cache_month.borrow()
    }
    
    pub fn load_last_spendings(&mut self, n: usize) ->
            impl Iterator<Item=MayLoad<'_>> {
        // self.trim_caches();
        // self.sync_upstream();
        
        self.live_records.1.iter().rev().map(|v| Ok((v.0, &v.1)))
            .chain(std::iter::repeat_n(Err(()), self.live_records.0))
            .take(n)
    }
    
    pub fn load_some_spendings(&mut self, rev_later: usize, rev_older: usize) ->
            impl Iterator<Item=MayLoad<'_>> {
        self.trim_caches();
        self.sync_upstream();
        
        assert!(rev_later <= rev_older);
        
        self.live_records.1.iter().rev().map(|v| Ok((v.0, &v.1)))
            .chain(std::iter::repeat_n(Err(()), self.live_records.0))
            .skip(rev_later).take(rev_older - rev_later)
    }
    
    pub fn insert_expense(&mut self, d: ClientData<'static>) {
        // We aren't gonna have ServerData like expense's index until upstream
        //   responds, but we must already show it.
        
        assert!(!d.revoked, "we shouldn't submit already-revoked records");
        
        // Generating ID assuming no one is writing in parallel.
        let provisional_id = self.total_live_transactions();
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
        self.provisional.insert(provisional_id, d.clone());
        
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
        
        self.pie_cache_month.trim_linear(|i| {
            let Some(i) = i.checked_sub(known.0) else {return None};
            let Some(expense) = known.1.get(i) else {return None};
            let (group, amount) = group_amount(expense);
            let expired = expense.server.time < liveline;
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
                    // We may need to remove a provisional record at this place.
                    if let Some(local_exp) = self.provisional.remove(&true_id) {
                        
                        // Do we need to adjust the caches?
                        if Some(true_id) != asked_id {
                            self.pie_cache_month.rewrite(
                                true_id,
                                group_amount_c(&local_exp),
                                group_amount(&expense),
                            );
                            self.pie_cache_forever.rewrite(
                                true_id,
                                group_amount_c(&local_exp),
                                group_amount(&expense),
                            );
                        }
                    } else if let Some(asked) = asked_id {
                        // Expense moved to a free place `true_id` from `asked`.
                        
                        self.pie_cache_month.shift(asked, true_id,
                            group_amount(&expense));
                        self.pie_cache_forever.shift(asked, true_id,
                            group_amount(&expense));
                    } else {
                        let (group, value) = group_amount(&expense);
                        self.pie_cache_month.push_back((group,value,true_id));
                        self.pie_cache_forever.push_back((group,value,true_id));
                    }
                    
                    assert_eq!(true_id, self.loaded_records.0
                                      + self.loaded_records.1.len());
                    self.loaded_records.1.push_back(expense.clone());
                    
                    if expense.client.revoked {continue;}
                    self.live_records.1.push_back((true_id, expense.clone()));
                },
            }
        }
    }
}


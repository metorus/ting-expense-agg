// #[sides(client)]

use time::OffsetDateTime;
use uuid::Uuid;

use std::collections::{BTreeMap, VecDeque};
use std::ops::Range;

use crate::crosstyping::{CachedStats, ClientData, Expense, MONTH_LIKE};
use crate::crosstyping::{Upstream, UpstreamMessage, DownstreamMessage};


const UNCLASSIFIED: &str = "unclassified";



#[derive(Clone, Copy)]
pub enum MayLoad<'a> {
    Confirmed(&'a Expense),
    Provisional { data: &'a ClientData, temp_time: OffsetDateTime },
    NotLoaded,
}

pub struct DbView<U: Upstream> {
    upstream: U,
    live_records: Vec<Expense>,
    provisional: Vec<(Uuid, ClientData, OffsetDateTime)>,
    life_stats: CachedStats,
    month_stats: CachedStats
}

impl<U: Upstream> DbView<U> {
    pub fn with(mut upstream: U) -> Self {
        let (life_stats, month_stats, live_records) = upstream.take_init();
        
        Self {
            upstream,
            live_records,
            provisional: Vec::with_capacity(1),
            life_stats,
            month_stats,
        }
    }

    fn keep_month(&mut self) -> OffsetDateTime {
        // todo!("check while oldest expense is over a month old");
        
        let now = OffsetDateTime::now_local().unwrap()
                                 .replace_nanosecond(0).unwrap();
        now - MONTH_LIKE
    }

    fn sync_upstream(&mut self) {
        let liveline = self.keep_month();
        for msg in self.upstream.sync() {
            match msg {
                UpstreamMessage::Revoked { expense } => {
                    self.handle_revocation(expense, liveline);
                }
                UpstreamMessage::NewSpending { expense, temp_alias } => {
                    self.apply_confirmed(expense, temp_alias, liveline);
                }
            }
        }
    }

    fn handle_revocation(&mut self, expense: Expense, liveline: OffsetDateTime) {
        let Ok(idx) = self.live_records.binary_search_by(|e|
            e.server.time.cmp(&expense.server.time)) else {return};
        self.live_records.remove(idx);
        
        let c = &expense.client;
        self.life_stats.raw_add(c.group.as_deref().unwrap_or(UNCLASSIFIED),
            -(c.amount as i64), -1);
        if expense.server.time >= liveline {
            self.month_stats.raw_add(c.group.as_deref()
                .unwrap_or(UNCLASSIFIED), -(c.amount as i64), -1);
        }
    }

    fn apply_confirmed(&mut self, expense: Expense, temp_alias: Uuid, liveline: OffsetDateTime) {
        assert!(!expense.client.revoked);
        
        let was_provisional = self.provisional.binary_search_by(|e|
            e.0.cmp(&temp_alias));

        if let Ok(idx) = was_provisional {
            self.provisional.remove(idx);
        } else {
            let amount = expense.client.amount;
            let group = &expense.client.group;
            
            self.life_stats.raw_add(group.as_deref()
                .unwrap_or(UNCLASSIFIED), amount as i64, 1);
            if expense.server.time >= liveline {
                self.month_stats.raw_add(group.as_deref()
                    .unwrap_or(UNCLASSIFIED), amount as i64, 1);
            }
        }

        let insert_pos = self.live_records.partition_point(|e|
            e.server.time < expense.server.time);
        self.live_records.insert(insert_pos, expense);
    }

    pub fn month_transactions_info(&mut self) -> (u64, usize) {
        (self.month_stats.total_spending, self.month_stats.records_alive)
    }

    pub fn life_transactions_info(&mut self) -> (u64, usize) {
        (self.life_stats.total_spending, self.life_stats.records_alive)
    }
    pub fn total_live_transactions(&mut self) -> usize {
        self.life_stats.records_alive
    }

    pub fn month_pie(&mut self) -> &[(String, u64)] {
        self.month_stats.group_spendings.as_slice()
    }

    pub fn life_pie(&mut self) -> &[(String, u64)] {
        self.life_stats.group_spendings.as_slice()
    }

    pub fn load_last_spendings(&mut self, n: usize) -> impl Iterator<Item = MayLoad<'_>> {
        let confirmed = self.live_records.iter().rev().map(MayLoad::Confirmed);
        let provisional = self.provisional.iter().rev()
            .map(|(_, d, t)| MayLoad::Provisional{data: d, temp_time: *t});
        
        let total_records = self.life_stats.records_alive;
        let have_records = self.live_records.len() + self.provisional.len();
        
        let missing = std::iter::repeat(MayLoad::NotLoaded)
            .take(total_records.saturating_sub(have_records));
            
        provisional.chain(confirmed).chain(missing).take(n)
    }

    pub fn load_some_spendings(&mut self, rev_from: usize, rev_to: usize) -> impl Iterator<Item = MayLoad<'_>> {
        let confirmed = self.live_records.iter().rev().map(MayLoad::Confirmed);
        let provisional = self.provisional.iter().rev()
            .map(|(_, d, t)| MayLoad::Provisional{data: d, temp_time: *t});
        
        let total_records = self.life_stats.records_alive;
        let have_records = self.live_records.len() + self.provisional.len();
        
        let missing = std::iter::repeat(MayLoad::NotLoaded)
            .take(total_records.saturating_sub(have_records));
            
        provisional.chain(confirmed).chain(missing)
            .skip(rev_from).take(rev_to - rev_from)
    }

    pub fn insert_expense(&mut self, c: ClientData) {
        assert!(!c.revoked);
        
        let now = OffsetDateTime::now_utc();
        let temp_alias = Uuid::now_v7();
        
        self.life_stats.raw_add(c.group.as_deref().unwrap_or(UNCLASSIFIED),
            (c.amount as i64), 1);
        self.month_stats.raw_add(c.group.as_deref().unwrap_or(UNCLASSIFIED),
            (c.amount as i64), 1);
        
        self.provisional.push((temp_alias, c.clone(), now));
        self.upstream.submit(DownstreamMessage::MadeExpense {
            info: c,
            temp_alias,
        });
    }
}


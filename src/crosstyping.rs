// #[sides(client, server)]

use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use std::borrow::Cow;
use uuid::Uuid;


pub const MONTH_LIKE: Duration = Duration::days(30);

//----------------------------------------------------------------------------//
/// Server-generated information about a certain expense.
/// If we intend to allow sharing information, these fields must not be
/// client-controllable at risk of falsification.
#[derive(Clone)]
pub struct Metadata<'de> {
    pub uid: Uuid,
    pub time: OffsetDateTime,
    
    #[expect(dead_code, reason="until server is introduced, why read?")]
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
        write!(f, "{:08X} - {} - {}P on {}",
            self.server.uid.as_fields().0,
            self.server.time.format(&Rfc3339).unwrap(),
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
                time: OffsetDateTime::now_local().unwrap(),
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



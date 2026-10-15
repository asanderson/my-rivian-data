//! One-use, short-lived approval records bound to the native account generation.
use serde_json::Value;
use std::time::{Duration, Instant};

pub(crate) struct Confirmation {
    pub id: String,
    pub operation_id: String,
    pub variables: Value,
    pub epoch: u64,
    pub expires: Instant,
}
#[derive(Default)]
pub(crate) struct Confirmations(Vec<Confirmation>);
impl Confirmations {
    pub fn issue(
        &mut self,
        id: String,
        operation_id: String,
        variables: Value,
        epoch: u64,
        now: Instant,
    ) -> bool {
        self.0.retain(|ticket| ticket.expires > now);
        if self.0.len() >= 8 || self.0.iter().any(|ticket| ticket.id == id) {
            return false;
        }
        self.0.push(Confirmation {
            id,
            operation_id,
            variables,
            epoch,
            expires: now + Duration::from_secs(120),
        });
        true
    }
    pub fn consume(&mut self, id: &str, epoch: u64, now: Instant) -> Option<Confirmation> {
        let index = self.0.iter().position(|ticket| ticket.id == id)?;
        let ticket = self.0.remove(index);
        (ticket.epoch == epoch && ticket.expires > now).then_some(ticket)
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn confirmation_runs_once_and_keeps_exact_variables() {
        let now = Instant::now();
        let mut book = Confirmations::default();
        let variables = json!({"vehicle_id":"vehicle-1","name":"Owner's truck"});
        assert!(book.issue(
            "nonce".into(),
            "rename-vehicle".into(),
            variables.clone(),
            3,
            now
        ));
        let ticket = book.consume("nonce", 3, now).unwrap();
        assert_eq!(ticket.variables, variables);
        assert!(book.consume("nonce", 3, now).is_none());
    }
    #[test]
    fn expiry_wrong_account_and_capacity_fail_closed() {
        let now = Instant::now();
        let mut book = Confirmations::default();
        assert!(book.issue("a".into(), "rename-vehicle".into(), json!({}), 3, now));
        assert!(book.consume("a", 4, now).is_none());
        assert!(book.consume("a", 3, now).is_none());
        assert!(book.issue("b".into(), "rename-vehicle".into(), json!({}), 3, now));
        assert!(
            book.consume("b", 3, now + Duration::from_secs(120))
                .is_none()
        );
        for n in 0..8 {
            assert!(book.issue(n.to_string(), "rename-vehicle".into(), json!({}), 3, now));
        }
        assert!(!book.issue("excess".into(), "rename-vehicle".into(), json!({}), 3, now));
        assert!(book.issue(
            "renew".into(),
            "rename-vehicle".into(),
            json!({}),
            3,
            now + Duration::from_secs(120)
        ));
    }
}

//! Private, process-owned delegation into the existing Windows camera coordinator.
//! The Flutter child never selects a revision, receives authority, or owns a guard.
use anyhow::{Context as _, Result, ensure};
use parking_lot::Mutex;
use std::{sync::{Arc, mpsc::{self, Receiver, SyncSender}}, time::{Duration, Instant}};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action { Block, Allow, Resume, RestoreLegacy { instance_id: String } }
impl Action {
    pub fn replaces_manual_intent(&self) -> bool { !matches!(self, Self::Resume) }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stamp {
    pub camera: u64,
    pub manual: u64,
    pub locked: bool,
    pub restore: bool,
}
#[derive(Clone, Copy, Debug, Default)]
struct Status { stamp: Stamp, available: bool }
#[derive(Clone, Debug)]
pub struct Client { requests: SyncSender<Request>, status: Arc<Mutex<Status>> }
pub struct Owner { requests: Receiver<Request>, status: Arc<Mutex<Status>> }
pub struct Request {
    pub action: Action,
    stamp: Stamp,
    expires: Instant,
    reply: SyncSender<std::result::Result<(), String>>,
}
pub struct Completion(SyncSender<std::result::Result<(), String>>);

pub fn channel() -> (Client, Owner) {
    let (requests, receiver) = mpsc::sync_channel(1);
    let status = Arc::new(Mutex::new(Status::default()));
    (Client { requests, status: Arc::clone(&status) }, Owner { requests: receiver, status })
}
impl Client {
    pub fn prepare(&self) -> Result<Stamp> {
        let state = *self.status.lock();
        ensure!(state.available, "Native camera coordinator is busy or unavailable");
        Ok(state.stamp)
    }
    pub fn execute(&self, action: Action, stamp: Stamp) -> Result<()> {
        let response = self.submit(action, stamp)?;
        response.recv_timeout(Duration::from_secs(120))
            .context("Native camera result unavailable; outcome unknown, refresh before another action")?
            .map_err(anyhow::Error::msg)
    }
    fn submit(&self, action: Action, stamp: Stamp) -> Result<Receiver<std::result::Result<(), String>>> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests.try_send(Request { action, stamp, expires: Instant::now() + Duration::from_secs(5), reply })
            .map_err(|_| anyhow::anyhow!("Native camera queue busy or disconnected; action not admitted"))?;
        Ok(response)
    }
}
impl Owner {
    pub fn publish(&self, stamp: Stamp, available: bool) { *self.status.lock() = Status { stamp, available }; }
    pub fn take(&self) -> Option<Request> { self.requests.try_recv().ok() }
    pub fn admit(&self, request: Request) -> Result<(Action, Completion)> {
        let state = *self.status.lock();
        if !state.available || state.stamp != request.stamp || Instant::now() > request.expires {
            let _ = request.reply.send(Err("Native camera coordinator changed, is busy, or confirmation expired; no action executed".into()));
            anyhow::bail!("Native camera admission refused");
        }
        // Busy remains set until the native result updates both ownership and state.
        self.status.lock().available = false;
        Ok((request.action, Completion(request.reply)))
    }
}
impl Completion {
    pub fn finish(self, result: std::result::Result<(), String>) { let _ = self.0.send(result); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_generation_busy_expiry_and_disconnected_owner_never_admit_an_effect() {
        let (client, owner) = channel();
        let stamp = Stamp::default();
        assert!(client.prepare().is_err());
        owner.publish(stamp, true);
        let response = client.submit(Action::Block, client.prepare().unwrap()).unwrap();
        owner.publish(Stamp { manual: 1, ..stamp }, true);
        assert!(owner.admit(owner.take().unwrap()).is_err());
        assert!(response.recv().unwrap().is_err());
        owner.publish(stamp, true);
        let response = client.submit(Action::Allow, stamp).unwrap();
        owner.publish(stamp, false);
        assert!(owner.admit(owner.take().unwrap()).is_err());
        assert!(response.recv().unwrap().is_err());
        owner.publish(stamp, true);
        let response = client.submit(Action::Resume, stamp).unwrap();
        let mut request = owner.take().unwrap();
        request.expires = Instant::now() - Duration::from_secs(1);
        assert!(owner.admit(request).is_err());
        assert!(response.recv().unwrap().is_err());
        drop(owner);
        assert!(client.submit(Action::Block, stamp).is_err());
    }
    #[test]
    fn native_completion_survives_frontend_disconnection_and_preserves_failure() {
        let (client, owner) = channel();
        owner.publish(Stamp::default(), true);
        let response = client.submit(Action::Resume, client.prepare().unwrap()).unwrap();
        let (action, completion) = owner.admit(owner.take().unwrap()).unwrap();
        assert_eq!(action, Action::Resume);
        assert!(!action.replaces_manual_intent());
        assert!(client.prepare().is_err());
        completion.finish(Err("UAC declined".into()));
        assert_eq!(response.recv().unwrap(), Err("UAC declined".into()));
        owner.publish(Stamp::default(), true);
        let response = client.submit(Action::Block, client.prepare().unwrap()).unwrap();
        let (action, completion) = owner.admit(owner.take().unwrap()).unwrap();
        assert!(action.replaces_manual_intent());
        drop(response);
        drop(client);
        completion.finish(Ok(())); // Native ownership does not depend on the child/receiver.
    }
}

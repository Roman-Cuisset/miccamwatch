//! Explicit quit admission. Closing Flutter never calls this process-owned lane.
use anyhow::{Result, ensure};
use serde::Serialize;
use std::{sync::{Arc, Mutex, mpsc::{self, Receiver, SyncSender}}, time::{Duration, Instant}};

#[derive(Default, Debug)]
struct State { ready: bool, revision: u64, requested: bool }
#[derive(Clone, Debug)]
pub struct Client { requests: SyncSender<Request>, state: Arc<Mutex<State>> }
pub struct Owner { requests: Receiver<Request>, state: Arc<Mutex<State>> }
struct Request { revision: u64, expires: Instant, reply: SyncSender<std::result::Result<(), String>> }
#[derive(Serialize)]
pub struct Status { pub ready: bool, pub requested: bool, pub scope: &'static str }
pub fn channel() -> (Client, Owner) {
    let (requests, receiver) = mpsc::sync_channel(1);
    let state = Arc::new(Mutex::new(State::default()));
    (Client { requests, state: Arc::clone(&state) }, Owner { requests: receiver, state })
}
impl Client {
    pub fn status(&self) -> Result<Status> {
        let state = self.state.lock().map_err(|_| anyhow::anyhow!("Native exit state unavailable"))?;
        Ok(Status { ready: state.ready && !state.requested, requested: state.requested,
            scope: "Explicitly quits this Rust owner using its native exit contract, after pending work drains. Not the same as closing Flutter. Persistent device guards are not blindly released or restored." })
    }
    pub fn prepare(&self) -> Result<u64> {
        let state = self.state.lock().map_err(|_| anyhow::anyhow!("Native exit state unavailable"))?;
        ensure!(state.ready && !state.requested, "Finish pending native operations before quitting");
        Ok(state.revision)
    }
    pub fn execute(&self, revision: u64) -> Result<()> {
        self.submit(revision)?.recv_timeout(Duration::from_secs(10))
            .map_err(|_| anyhow::anyhow!("Native quit admission unavailable; exit not confirmed"))?
            .map_err(anyhow::Error::msg)
    }
    fn submit(&self, revision: u64) -> Result<Receiver<std::result::Result<(), String>>> {
        let (reply, result) = mpsc::sync_channel(1);
        self.requests.try_send(Request { revision, expires: Instant::now() + Duration::from_secs(5), reply })
            .map_err(|_| anyhow::anyhow!("Native exit queue busy or disconnected"))?;
        Ok(result)
    }
}
impl Owner {
    /// `ready` is the existing native exit predicate, excluding only this lane's
    /// own consultation ticket. The actual native exit is checked again later.
    pub fn poll(&self, ready: bool) -> bool {
        let Ok(mut state) = self.state.lock() else { return false };
        if state.ready != ready {
            let Some(next) = state.revision.checked_add(1) else { state.ready = false; return false };
            state.revision = next;
            state.ready = ready;
        }
        let Ok(request) = self.requests.try_recv() else { return false };
        if !ready || state.requested || state.revision != request.revision || Instant::now() > request.expires {
            let _ = request.reply.send(Err("Native exit refused: busy, stale or expired; engine and protections preserved".into()));
            return false;
        }
        state.requested = true;
        let _ = request.reply.send(Ok(()));
        true
    }
    pub fn requested(&self) -> bool { self.state.lock().is_ok_and(|state| state.requested) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_native_work_stale_confirmation_and_duplicate_quit_are_refused() {
        let (client, owner) = channel();
        assert!(client.prepare().is_err());
        owner.poll(true);
        let revision = client.prepare().unwrap();
        let response = client.submit(revision).unwrap();
        assert!(!owner.poll(false));
        assert!(response.recv().unwrap().is_err());
        owner.poll(true);
        let response = client.submit(revision).unwrap();
        assert!(!owner.poll(true));
        assert!(response.recv().unwrap().is_err());
        let response = client.submit(client.prepare().unwrap()).unwrap();
        assert!(owner.poll(true));
        assert!(response.recv().unwrap().is_ok());
        assert!(owner.requested());
        assert!(client.prepare().is_err());
        let response = client.submit(revision).unwrap();
        assert!(!owner.poll(true));
        assert!(response.recv().unwrap().is_err());
    }
    #[test]
    fn frontend_drop_does_not_request_exit_and_an_admitted_quit_is_not_cancelled_by_it() {
        let (client, owner) = channel();
        owner.poll(true);
        drop(client.clone());
        assert!(!owner.requested());
        let response = client.submit(client.prepare().unwrap()).unwrap();
        assert!(owner.poll(true));
        drop(response);
        drop(client);
        assert!(owner.requested());
    }
    #[test]
    fn expired_or_disconnected_quit_does_not_admit_shutdown() {
        let (client, owner) = channel();
        owner.poll(true);
        let response = client.submit(client.prepare().unwrap()).unwrap();
        let mut request = owner.requests.try_recv().unwrap();
        request.expires = Instant::now() - Duration::from_secs(1);
        assert!(client.requests.try_send(request).is_ok());
        assert!(!owner.poll(true));
        assert!(response.recv().unwrap().is_err());
        assert!(!owner.requested());
        drop(owner);
        assert!(client.execute(0).is_err());
    }
}

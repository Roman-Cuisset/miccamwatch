//! Explicit local diagnostic sampling; never enabled in ordinary builds/runs.
//! Inclusive scopes nest: their CPU totals must not be added together.
use std::{marker::PhantomData, rc::Rc, sync::LazyLock, time::Instant};
use windows::Win32::{Foundation::FILETIME, System::Threading::{GetCurrentThread, GetThreadTimes}};

static ENABLED: LazyLock<bool> = LazyLock::new(|| cfg!(feature = "flutter-desktop") &&
    std::env::var_os("MCW_COLLECTOR_PROFILE").as_deref() == Some(std::ffi::OsStr::new("1")));
fn enabled() -> bool { *ENABLED }
fn thread_cpu() -> Option<u64> {
    let (mut created, mut exited, mut kernel, mut user) =
        (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    unsafe { GetThreadTimes(GetCurrentThread(), &mut created, &mut exited, &mut kernel, &mut user) }.ok()?;
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    Some(ticks(kernel) + ticks(user))
}
pub(super) struct Span {
    stage: &'static str,
    sample: Option<(Instant, u64)>,
    same_thread: PhantomData<Rc<()>>,
}
impl Span {
    pub(super) fn new(stage: &'static str) -> Self {
        Self { stage, sample: enabled().then(|| thread_cpu().map(|cpu| (Instant::now(), cpu))).flatten(), same_thread: PhantomData }
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some((start, cpu)) = self.sample {
            if let Some(end) = thread_cpu() {
                eprintln!("{{\"event\":\"collector_profile\",\"stage\":\"{}\",\"cpu_100ns\":{},\"wall_ns\":{}}}",
                    self.stage, end.saturating_sub(cpu), start.elapsed().as_nanos());
            } else {
                eprintln!("{{\"event\":\"collector_profile_unavailable\",\"stage\":\"{}\"}}", self.stage);
            }
        }
    }
}

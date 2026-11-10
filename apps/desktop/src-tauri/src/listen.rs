//! The sync listener's accept loop. It runs on its own thread and hands
//! each connection to the session thread only once the peer has sent its
//! first bytes, so a silent connection never queues ahead of the user's
//! commands. `wugs_net::serve_stream` bounds everything after that.

use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use wugs_auth::DeviceKey;

use crate::session::{self, ServedSession, SyncListener, FIRST_BYTE_WAIT};
use crate::SessionThread;

/// How often the accept loop looks at its stop flag when no one connects.
const POLL: Duration = Duration::from_millis(50);
/// The pause after a failed accept (out of file descriptors, say), so a
/// persistent error does not spin a core.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(250);

/// A running sync listener. Dropping it stops the accept loop and closes
/// the port before `drop` returns.
pub struct Listening {
    pub project_id: String,
    pub device_id: String,
    pub port: u16,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.accept.take() {
            t.join().ok();
        }
    }
}

/// Serve every connection to `l` on the session thread; `on_served` hears
/// each finished session. Connections that stay silent are dropped unheard.
pub fn start(
    thread: SessionThread,
    key: Arc<DeviceKey>,
    l: SyncListener,
    on_served: impl Fn(ServedSession) + Send + Sync + 'static,
) -> Result<Listening, String> {
    let port = l.listener.local_addr().map_err(|e| e.to_string())?.port();
    l.listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let on_served = Arc::new(on_served);
    let (flag, project_id) = (stop.clone(), l.project_id.clone());
    let listener = l.listener;
    let accept = std::thread::Builder::new()
        .name("wugs-sync-accept".into())
        .spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                let conn = match listener.accept() {
                    Ok((conn, _)) => conn,
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(POLL);
                        continue;
                    }
                    Err(_) => {
                        std::thread::sleep(ACCEPT_BACKOFF);
                        continue;
                    }
                };
                let (thread, key, project_id, on_served) = (thread.clone(), key.clone(), project_id.clone(), on_served.clone());
                std::thread::spawn(move || {
                    // Some platforms hand out accepted sockets in the listener's mode.
                    if conn.set_nonblocking(false).is_err() || wugs_net::wait_for_first_byte(&conn, FIRST_BYTE_WAIT).is_err() {
                        return;
                    }
                    let served = thread.run(move |s| session::serve_connection(s, &key, &project_id, conn));
                    on_served(served.unwrap_or_else(|e| ServedSession { peer_label: None, outcome: e, pulled_ops: 0, warnings: Vec::new() }));
                });
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(Listening { project_id: l.project_id, device_id: l.device_id, port, stop, accept: Some(accept) })
}

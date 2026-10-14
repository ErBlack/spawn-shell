use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::flow::{Gate, Wanted};
use crate::pty;

const TERMINATE_GRACE: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ownership {
    Child,
    Adopted,
}

pub(super) struct Proc {
    pub(super) pid: i32,
    pub(super) master: OwnedFd,
    pub(super) input: mpsc::Sender<Vec<u8>>,
    pub(super) gate: Gate,
    pub(super) ownership: Ownership,
}

fn alive(pid: i32, ownership: Ownership) -> bool {
    match ownership {
        Ownership::Child => pty::child_running(pid),
        Ownership::Adopted => pty::exists(pid),
    }
}

pub(super) fn hang_up(
    pid: i32,
    group: Option<i32>,
    ownership: Ownership,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        if let Some(group) = group.filter(|&group| group != pid) {
            pty::signal(-group, libc::SIGHUP);
        }
        pty::signal(-pid, libc::SIGHUP);
        let deadline = Instant::now() + TERMINATE_GRACE;
        while Instant::now() < deadline {
            if !alive(pid, ownership) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        pty::signal(-pid, libc::SIGKILL);
        if ownership == Ownership::Child {
            pty::reap(pid);
        }
    })
}

impl Proc {
    pub(super) fn terminate(self) -> thread::JoinHandle<()> {
        self.gate.want(Wanted::Stop);
        let group = pty::foreground(self.master.as_raw_fd());
        hang_up(self.pid, group, self.ownership)
    }

    pub(super) fn finished(self) {
        self.gate.want(Wanted::Stop);
        if self.ownership == Ownership::Child {
            thread::spawn(move || pty::reap(self.pid));
        }
    }
}

use std::sync::Arc;
use std::time::Instant;

use parking_lot::{Condvar, Mutex};

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Wanted {
    #[default]
    Run,
    Pause,
    Stop,
}

impl Wanted {
    pub(super) fn paused(paused: bool) -> Self {
        if paused { Wanted::Pause } else { Wanted::Run }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Reader {
    #[default]
    Reading,
    Parked,
    Finished,
}

#[derive(Default)]
struct Flow {
    wanted: Wanted,
    held: bool,
    reader: Reader,
}

impl Flow {
    fn current(&self) -> Wanted {
        match self.wanted {
            Wanted::Stop => Wanted::Stop,
            _ if self.held => Wanted::Pause,
            wanted => wanted,
        }
    }
}

#[derive(Default, Clone)]
pub(super) struct Gate(Arc<(Mutex<Flow>, Condvar)>);

impl Gate {
    pub(super) fn new(wanted: Wanted) -> Self {
        let gate = Gate::default();
        gate.want(wanted);
        gate
    }

    pub(super) fn want(&self, wanted: Wanted) {
        let (flow, signal) = &*self.0;
        let mut state = flow.lock();
        if state.wanted != Wanted::Stop {
            state.wanted = wanted;
        }
        signal.notify_all();
    }

    pub(super) fn hold(&self, held: bool) {
        let (flow, signal) = &*self.0;
        flow.lock().held = held;
        signal.notify_all();
    }

    pub(super) fn current(&self) -> Wanted {
        self.0.0.lock().current()
    }

    pub(super) fn wait_until_resumed(&self) -> bool {
        let (flow, signal) = &*self.0;
        let mut state = flow.lock();
        if state.current() == Wanted::Pause {
            state.reader = Reader::Parked;
            signal.notify_all();
            while state.current() == Wanted::Pause {
                signal.wait(&mut state);
            }
            state.reader = Reader::Reading;
        }
        state.current() != Wanted::Stop
    }

    pub(super) fn reader_finished(&self) {
        let (flow, signal) = &*self.0;
        flow.lock().reader = Reader::Finished;
        signal.notify_all();
    }

    pub(super) fn wait_parked(&self, deadline: Instant) -> bool {
        let (flow, signal) = &*self.0;
        let mut state = flow.lock();
        while state.reader == Reader::Reading {
            if signal.wait_until(&mut state, deadline).timed_out() {
                return state.reader != Reader::Reading;
            }
        }
        true
    }
}

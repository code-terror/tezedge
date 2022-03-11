// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{any::Any, sync::mpsc, thread, time::Duration};

use crate::{
    machine::action::{Action, TimeoutAction, TimeoutDelayedAction},
    types::Timestamp,
};

pub struct Timer {
    task_sender: mpsc::Sender<Task>,
    handle: thread::JoinHandle<()>,
}

impl Timer {
    pub fn spawn(action_sender: mpsc::Sender<Action>) -> Self {
        let (task_sender, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut d = None;
            loop {
                let next = match d.take() {
                    Some((d, delayed)) => match rx.recv_timeout(d) {
                        Ok(next) => next,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let action = if delayed {
                                Action::TimeoutDelayed(TimeoutDelayedAction {
                                    now_timestamp: Timestamp::now().0,
                                })
                            } else {
                                Action::Timeout(TimeoutAction {
                                    now_timestamp: Timestamp::now().0,
                                })
                            };
                            let _ = action_sender.send(action);
                            continue;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    },
                    None => match rx.recv() {
                        Ok(next) => next,
                        Err(mpsc::RecvError) => break,
                    },
                };
                match next {
                    Task::First(timestamp) => {
                        d = Some((timestamp.duration_from_now(), false));
                    }
                    Task::Next(duration) => {
                        d = Some((duration, true));
                    }
                }
            }
        });
        Timer {
            task_sender,
            handle,
        }
    }

    pub fn timeout<F>(&self, timestamp: Timestamp, wrapper: F)
    where
        F: Fn(TimeoutAction) -> Action + Send + 'static,
    {
        let _ = wrapper;
        let _ = self.task_sender.send(Task::First(timestamp));
    }

    #[allow(dead_code)]
    pub fn next_timeout<F>(&self, duration: Duration, wrapper: F)
    where
        F: Fn(TimeoutDelayedAction) -> Action + Send + 'static,
    {
        let _ = wrapper;
        let _ = self.task_sender.send(Task::Next(duration));
    }

    pub fn join(self) -> Result<(), Box<dyn Any + Send + 'static>> {
        drop(self.task_sender);
        self.handle.join()
    }
}

enum Task {
    First(Timestamp),
    Next(Duration),
}
use crate::app::Task;
use crate::app::{ExternalMsg, InternalMsg, MsgIn};
use crate::input::Key;
use crossterm::event::{self, Event};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

pub fn keep_reading(tx_msg_in: Sender<Task>, rx_event_reader: Receiver<bool>) {
    thread::spawn(move || {
        let mut is_paused = false;
        loop {
            if let Some(paused) = rx_event_reader.try_recv().ok() {
                is_paused = paused;
            };

            if !is_paused {
                if event::poll(std::time::Duration::from_millis(1)).unwrap() {
                    match event::read().unwrap() {
                        Event::Key(key) => {
                            let key = Key::from_event(key);
                            let msg = MsgIn::Internal(InternalMsg::HandleKey(key));
                            tx_msg_in.send(Task::new(0, msg, Some(key))).unwrap();
                        }

                        Event::Resize(_, _) => {
                            let msg = MsgIn::External(ExternalMsg::Refresh);
                            tx_msg_in.send(Task::new(0, msg, None)).unwrap();
                        }
                        _ => {}
                    }
                }
            }
        }
    });
}

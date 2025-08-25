use std::hint::spin_loop;
use std::sync::mpsc::TrySendError;
use std::sync::mpsc::{Receiver, SyncSender};
use std::thread::sleep;
use std::time::{Duration, Instant};
use mmnes_core::key_event::KeyEvents;
use mmnes_core::nes_console::{NesConsole, NesConsoleError};
use mmnes_core::nes_frame::NesFrame;
use mmnes_core::nes_samples::NesSamples;
use crate::{FRAMES_PER_SECOND, SPIN_BEFORE};
use crate::sound_player::SoundPlayer;

pub struct NesFrontEnd {
    rx: Receiver<KeyEvents>,
    tx: SyncSender<NesFrame>,
    nes: NesConsole
}

impl NesFrontEnd {
    
    pub fn new(nes: NesConsole, tx: SyncSender<NesFrame>, rx: Receiver<KeyEvents>) -> NesFrontEnd {
        NesFrontEnd {
            nes,
            tx,
            rx,
        }
    }

    fn sleep_until_next_frame(next: Instant, frame: Duration) -> Instant {
        let now = Instant::now();
        let mut next = next;

        if next > now {
            let mut to_sleep = next - now;
            if to_sleep > SPIN_BEFORE {
                to_sleep -= SPIN_BEFORE;
                sleep(to_sleep);
            }

            while Instant::now() < next {
                spin_loop();
            }

            next + frame
        } else {
            while next <= now {
                next += frame;
            }

            next
        }
    }

    fn get_input(&self) -> Option<KeyEvents> {
        let mut acc = KeyEvents::new();

        while let Ok(events) = self.rx.try_recv() {
            acc = acc.chain(events).collect();
        }

        (!acc.is_empty()).then_some(acc)
    }

    fn process_frame(&self, frame: NesFrame) -> Result<(), NesConsoleError> {
        match self.tx.try_send(frame) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_frame)) => Ok(()), // drop frame
            Err(TrySendError::Disconnected(frame)) => {
                Err(NesConsoleError::ChannelCommunication(format!("UI is gone ... frame {}", frame.count())))
            }
        }
    }

    fn process_samples(&self, samples: NesSamples, sound_player: &mut SoundPlayer) -> Result<(), NesConsoleError> {
        for sample in samples.samples() {
            sound_player.push_sample(*sample)
        }

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), NesConsoleError> {
        let frame_duration = Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND);
        let mut next_frame = Instant::now() + frame_duration;
        let mut sound_player = SoundPlayer::new().map_err(|e| NesConsoleError::ControllerError(e.to_string()))?;

        loop {
            let inputs = self.get_input();

            if let Some(inputs) = inputs {
                self.nes.set_input(inputs)?;
            }

            let (frame, samples) = self.nes.step_frame()?;
            self.process_frame(frame)?;
            self.process_samples(samples, &mut sound_player)?;

            next_frame = NesFrontEnd::sleep_until_next_frame(next_frame, frame_duration);
        }
    }
}
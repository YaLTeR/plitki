//! The audio engine.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{OutputCallbackInfo, SampleFormat, Stream};
use crossbeam_channel::{Receiver, Sender};
use rodio::cpal::StreamConfig;
use rodio::source::UniformSourceIterator;
use rodio::{cpal, Sample, Source};
use triple_buffer::TripleBuffer;

/// Message sent to the audio thread.
enum ToAudioMessage {
    /// Play the next track.
    Play {
        /// Track to play.
        track: Box<dyn Source<Item = f32> + Send>,
        /// Identifier of the track.
        id: usize,
    },
}

/// The main struct managing the audio playback.
pub struct AudioEngine {
    _stream: Stream,
    config: StreamConfig,
    timestamp_consumer: RefCell<triple_buffer::Output<Option<AudioTimestamp>>>,
    sender: Sender<ToAudioMessage>,
    current_track_id: Cell<usize>,
}

impl std::fmt::Debug for AudioEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioManager").finish()
    }
}

impl AudioEngine {
    /// Creates a new [`AudioEngine`].
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("no output device available");
        let config = device
            .default_output_config()
            .expect("could not pick output config");
        debug!("using device {:?} with config {:?}", device.name(), config);

        let stream_config = config.config();
        let on_error = move |err| {
            error!("audio error: {err:?}");
        };

        let (timestamp_producer, timestamp_consumer) = TripleBuffer::new(&None).split();
        let (sender, receiver) = crossbeam_channel::unbounded();

        let state = AudioThreadState::new(stream_config.clone(), timestamp_producer, receiver);

        let stream = match config.sample_format() {
            SampleFormat::I16 => {
                device.build_output_stream(&stream_config, state.into_callback::<i16>(), on_error)
            }
            SampleFormat::U16 => {
                device.build_output_stream(&stream_config, state.into_callback::<u16>(), on_error)
            }
            SampleFormat::F32 => {
                device.build_output_stream(&stream_config, state.into_callback::<f32>(), on_error)
            }
        }
        .expect("could not build output stream");
        stream.play().expect("could not play stream");

        Self {
            _stream: stream,
            config: stream_config,
            timestamp_consumer: RefCell::new(timestamp_consumer),
            sender,
            current_track_id: Cell::new(0),
        }
    }

    /// Starts playing the `track`.
    ///
    /// After calling this method, [`AudioEngine::track_time()`] may return [`Duration::ZERO`] for a
    /// little bit, until the track actually starts playing.
    pub fn play_track(&self, track: impl Source<Item = impl Sample + Send> + Send + 'static) {
        // Do all these allocations here, rather than on the audio thread.
        let track =
            UniformSourceIterator::new(track, self.config.channels, self.config.sample_rate.0);
        let track = Box::new(track);

        self.current_track_id.set(self.current_track_id.get() + 1);

        let message = ToAudioMessage::Play {
            track,
            id: self.current_track_id.get(),
        };
        if let Err(err) = self.sender.send(message) {
            error!("error sending track to audio thread: {err:?}");
        }
    }

    /// Returns current playback position of the track.
    ///
    /// The playback position will keep increasing past the end of the track (until another track is
    /// started with [`AudioEngine::play_track()`]).
    pub fn track_time(&self) -> Duration {
        let mut timestamp_consumer = self.timestamp_consumer.borrow_mut();

        let AudioTimestamp {
            track_id,
            track_timestamp,
            will_play_at,
        } = match *timestamp_consumer.read() {
            Some(x) => x,
            None => return Duration::ZERO,
        };

        if track_id != self.current_track_id.get() {
            // This timestamp is still stale: the audio thread hasn't got to the latest track yet.
            return Duration::ZERO;
        }

        let now = Instant::now();
        if let Some(time_until_played) = will_play_at.checked_duration_since(now) {
            track_timestamp.saturating_sub(time_until_played)
        } else {
            let time_since_played = now.duration_since(will_play_at);
            track_timestamp + time_since_played
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// A timestamp from the audio thread.
///
/// Indicates that the track timestamp [`AudioTimestamp::track_timestamp`] will play at
/// [`AudioTimestamp::will_play_at`] time.
#[derive(Debug, Clone, Copy)]
struct AudioTimestamp {
    /// Identifier of the track this timestamp is for.
    track_id: usize,
    track_timestamp: Duration,
    will_play_at: Instant,
}

struct AudioThreadState {
    config: StreamConfig,

    /// Source providing silent samples according to [`AudioThreadState::config`].
    silence: rodio::source::Zero<f32>,

    /// Audio that's currently playing.
    ///
    /// Should have the same sample rate and channel count as [`AudioThreadState::config`].
    track: Box<dyn Source<Item = f32> + Send>,

    /// Identifier of the track that's currently playing.
    track_id: usize,

    /// Total number of samples taken from [`AudioThreadState::track`].
    samples_taken: usize,

    timestamp_producer: triple_buffer::Input<Option<AudioTimestamp>>,

    receiver: Receiver<ToAudioMessage>,
}

impl AudioThreadState {
    fn new(
        config: StreamConfig,
        timestamp_producer: triple_buffer::Input<Option<AudioTimestamp>>,
        receiver: Receiver<ToAudioMessage>,
    ) -> Self {
        let silence = rodio::source::Zero::new(config.channels, config.sample_rate.0);

        Self {
            config,
            silence: silence.clone(),
            track: Box::new(silence),
            samples_taken: 0,
            timestamp_producer,
            receiver,
            track_id: usize::MAX,
        }
    }

    fn into_callback<S: Sample>(mut self) -> impl FnMut(&mut [S], &OutputCallbackInfo) {
        move |data, info| self.data_callback(data, info)
    }

    fn data_callback<S: Sample>(&mut self, data: &mut [S], info: &OutputCallbackInfo) {
        // Get current instant as soon as possible because it corresponds to the callback timestamp.
        let now = Instant::now();

        self.receive_messages();

        self.update_timestamp(info, now);

        let source = self.track.as_mut().chain(self.silence.clone());
        for (out, sample) in data.iter_mut().zip(source) {
            *out = S::from(&sample);
        }
        self.samples_taken += data.len();
    }

    fn update_timestamp(&mut self, info: &OutputCallbackInfo, callback_called_at: Instant) {
        let timestamp = info.timestamp();
        let time_until_played = timestamp
            .playback
            .duration_since(&timestamp.callback)
            .unwrap_or_else(|| {
                error!("cpal playback timestamp < callback timestamp, how is this possible?");
                Duration::ZERO
            });
        let track_timestamp = Duration::from_secs_f64(
            self.samples_taken as f64
                / self.config.sample_rate.0 as f64
                / self.config.channels as f64,
        );

        self.timestamp_producer.write(Some(AudioTimestamp {
            track_id: self.track_id,
            track_timestamp,
            will_play_at: callback_called_at + time_until_played,
        }));
    }

    fn receive_messages(&mut self) {
        if let Some(message) = self.receiver.try_iter().last() {
            match message {
                ToAudioMessage::Play { track, id } => {
                    self.track = track;
                    self.track_id = id;
                    self.samples_taken = 0;
                }
            }
        }
    }
}

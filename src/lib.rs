/*!
## Compose and play 3D audio.

The ambisonic library provides 3D sound scene support on top of [`rodio`](https://crates.io/crates/rodio).
It allows positioning and moving sound sources freely in 3D space around a virtual listener,
and playing the resulting spatial mix in real-time over a sound card.

### Features:
- Realistic directional audio
- Take `rodio` sound sources and place them in space
- Doppler effect on moving sounds

## Usage Example

```no_run
use std::thread::sleep;
use std::time::Duration;
use ambisonic::{rodio, AmbisonicBuilder};

let scene = AmbisonicBuilder::default().build();

let source = rodio::source::SineWave::new(440);
let mut sound = scene.play_at(source, [50.0, 1.0, 0.0]);

// move sound from right to left
sound.set_velocity([-10.0, 0.0, 0.0]);
for i in 0..1000 {
    sound.adjust_position([50.0 - i as f32 / 10.0, 1.0, 0.0]);
    sleep(Duration::from_millis(10));
}
sound.set_velocity([0.0, 0.0, 0.0]);
```

### Technical Details

`ambisonic` is built around the concept of an intermediate representation of the sound field,
called *B-format*. The *B-format* describes what the listener should hear, independent of
their audio playback equipment. This leads to a clear separation of audio scene composition and
rendering. For details, see [Wikipedia](https://en.wikipedia.org/wiki/Ambisonics).

In its current state, the library allows spatial composition of single-channel `rodio` sources
into a first-order *B-format* stream. The chosen renderer then decodes the *B-format* stream
into audio signals for playback.

Currently, the following renderers are available:

- Stereo: simple and efficient playback on two stereo speakers or headphones
- HRTF: realistic 3D sound over headphones using head related transfer functions

Although at the moment only stereo output is supported, the *B-format* abstraction should make
it easy to implement arbitrary speaker configurations in the future.
*/

mod bformat;
mod bmixer;
mod bstream;
mod renderer;

pub mod constants;
pub mod sources;
pub use bmixer::{bmixer, BmixerComposer, BstreamMixer};
pub use bstream::{bstream, Bstream, BstreamConfig, SoundController};
use renderer::MultiSpeakerConfig;
pub use renderer::{BstreamHrtfRenderer, BstreamStereoRenderer, HrtfConfig, StereoConfig};
pub use rodio;

use std::f32;
use std::sync::Arc;

/// Configure playback parameters
pub enum PlaybackConfiguration {
    /// Stereo playback
    Stereo(StereoConfig),

    /// Headphone playback using head related transfer functions
    Hrtf(HrtfConfig),

    MultiSpeaker(MultiSpeakerConfig),
}

impl Default for PlaybackConfiguration {
    fn default() -> Self {
        PlaybackConfiguration::Stereo(StereoConfig::default())
    }
}

impl From<StereoConfig> for PlaybackConfiguration {
    fn from(cfg: StereoConfig) -> Self {
        PlaybackConfiguration::Stereo(cfg)
    }
}

impl From<HrtfConfig> for PlaybackConfiguration {
    fn from(cfg: HrtfConfig) -> Self {
        PlaybackConfiguration::Hrtf(cfg)
    }
}

/// A builder object for creating `Ambisonic` contexts
pub struct AmbisonicBuilder {
    device: Option<rodio::Device>,
    sample_rate: u32,
    config: PlaybackConfiguration,
}

impl AmbisonicBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the ambisonic context
    pub fn build(self) -> Ambisonic {
        let (stream, stream_handle) = if let Some(device) = self.device {
            rodio::OutputStream::try_from_device(&device).unwrap()
        } else {
            rodio::OutputStream::try_default().unwrap()
        };

        let sink = rodio::Sink::try_new(&stream_handle).unwrap();

        let (mixer, controller) = bmixer::bmixer(self.sample_rate);

        match self.config {
            PlaybackConfiguration::Stereo(cfg) => {
                let output = renderer::BstreamStereoRenderer::new(mixer, cfg);
                sink.append(output);
            }

            PlaybackConfiguration::Hrtf(cfg) => {
                let output = renderer::BstreamHrtfRenderer::new(mixer, cfg);
                sink.append(output);
            }

            PlaybackConfiguration::MultiSpeaker(cfg) => {
                let output = renderer::BstreamMultiSpeakerRenderer::
            }
        }

        Ambisonic {
            sink,
            output_stream: stream,
            composer: controller,
        }
    }

    /// Select device (defaults to `rodio::default_output_device()`
    pub fn with_device(self, device: rodio::Device) -> Self {
        AmbisonicBuilder {
            device: Some(device),
            ..self
        }
    }

    /// Set sample rate fo the ambisonic mix
    pub fn with_sample_rate(self, sample_rate: u32) -> Self {
        AmbisonicBuilder {
            sample_rate,
            ..self
        }
    }

    /// Set playback configuration
    pub fn with_config(self, config: PlaybackConfiguration) -> Self {
        AmbisonicBuilder { config, ..self }
    }
}

impl Default for AmbisonicBuilder {
    fn default() -> Self {
        AmbisonicBuilder {
            device: None,
            sample_rate: 48000,
            config: PlaybackConfiguration::default(),
        }
    }
}

/// High-level Ambisonic Context.
///
/// Stops playing all sounds when dropped.
pub struct Ambisonic {
    // We need to hold on to Sink and Stream to keep the Audio alive
    #[allow(dead_code)]
    sink: rodio::Sink,
    #[allow(dead_code)]
    output_stream: rodio::OutputStream,

    composer: Arc<BmixerComposer>,
}

impl Ambisonic {
    /// Add a single-channel `Source` to the sound scene at a position relative to the listener
    ///
    /// Returns a controller object that can be used to control the source during playback.
    #[deprecated(
        since = "0.3.0",
        note = "please use one of the `play_*` methods instead"
    )]
    #[inline(always)]
    pub fn play<I>(&self, input: I) -> SoundController
    where
        I: rodio::Source<Item = f32> + Send + 'static,
    {
        self.play_omni(input)
    }

    /// Add a single-channel `Source` to the sound scene, initialized as omnidirectional.
    ///
    /// Returns a controller object that can be used to control the source during playback.
    #[inline(always)]
    pub fn play_omni<I>(&self, input: I) -> SoundController
    where
        I: rodio::Source<Item = f32> + Send + 'static,
    {
        self.composer.play(input, BstreamConfig::new())
    }

    /// Add a single-channel `Source` to the sound scene, initialized as omnidirectional.
    ///
    /// Returns a controller object that can be used to control the source during playback.
    #[inline(always)]
    pub fn play_at<I>(&self, input: I, pos: [f32; 3]) -> SoundController
    where
        I: rodio::Source<Item = f32> + Send + 'static,
    {
        self.composer
            .play(input, BstreamConfig::new().with_position(pos))
    }
}

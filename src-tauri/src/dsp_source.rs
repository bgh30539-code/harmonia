//! A `rodio::Source` adapter that runs the [`harmonia_core::dsp::EqChain`]
//! (ten-band EQ + bass shelf) over the decoded stream.
//!
//! One [`EqChain`] is kept per channel because biquad IIR state must not be
//! shared between channels. Filter coefficients are computed at the source's
//! sample rate (the mixer resamples to the device rate afterwards).

use std::time::Duration;

use harmonia_core::dsp::EqChain;
use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};

pub struct EqualizerSource<I> {
    inner: I,
    chains: Vec<EqChain>,
    frame: usize,
}

impl<I: Source> EqualizerSource<I> {
    pub fn new(inner: I, gains: Vec<f32>, bass_boost_db: f32) -> Self {
        let sample_rate = inner.sample_rate().get() as f32;
        let channels = inner.channels().get() as usize;
        let chains = (0..channels)
            .map(|_| EqChain::new(sample_rate, &gains, bass_boost_db))
            .collect();
        Self {
            inner,
            chains,
            frame: 0,
        }
    }
}

impl<I: Source> Iterator for EqualizerSource<I> {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        let sample = self.inner.next()?;
        let channels = self.chains.len();
        let index = self.frame % channels;
        self.frame += 1;
        Some(self.chains[index].process(sample))
    }
}

impl<I: Source> Source for EqualizerSource<I> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        // Seeking lands on a frame boundary; the channel index may now point
        // at a different channel, so it is safe to reset the filter state.
        self.frame = 0;
        for chain in &mut self.chains {
            chain.reset();
        }
        self.inner.try_seek(pos)
    }
}

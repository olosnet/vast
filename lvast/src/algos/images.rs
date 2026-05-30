//! Raw image statistics, histogram, and stretch helpers.
//!
//! This module operates on [`ImageFrame`] values using the crate's image-local
//! [`StandardImageFrameFormat`] model. It provides channel statistics,
//! histogram construction, auto-stretch estimation, midtones transfer, and a
//! simple RGB histogram visualization for raw frames.
//!
//! Algorithms here are inspired by KStars FITS viewer heuristics, especially
//! histogram binning and simple auto-stretch windows based on robust image
//! statistics. Multi-channel paths currently assume planar channel layout for
//! `RGB24` and `RGB32` frames.

use crate::{
    base::errors::{VastError, VastErrorType, VastResult},
    types::imageformats::{ImageFrame, ImageFrameFormat, StandardImageFrameFormat},
};

#[derive(Debug, Clone, PartialEq)]
/// Summary statistics for one image channel.
pub struct ImageChannelStats {
    /// Smallest sample value in channel.
    pub min: f64,
    /// Largest sample value in channel.
    pub max: f64,
    /// Arithmetic mean of all samples.
    pub mean: f64,
    /// Median sample value.
    pub median: f64,
    /// Population standard deviation.
    pub stddev: f64,
    /// Median absolute deviation from median.
    pub mad: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Statistics for full frame split into planar channels.
pub struct RawImageStats {
    /// Number of logical channels in frame format.
    pub channels: usize,
    /// Pixel count for each channel.
    pub pixels_per_channel: usize,
    /// Per-channel summary statistics.
    pub channel_stats: Vec<ImageChannelStats>,
}

#[derive(Debug, Clone, PartialEq)]
/// Histogram data for one channel.
pub struct HistogramChannel {
    /// Bin start intensity values.
    pub intensities: Vec<f64>,
    /// Per-bin sample counts.
    pub frequencies: Vec<u32>,
    /// Running total of frequencies.
    pub cumulative_frequencies: Vec<u32>,
    /// Intensity range covered by one bin.
    pub bin_width: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Histogram data for full frame.
pub struct RawHistogram {
    /// Number of bins used for each channel.
    pub bin_count: usize,
    /// Per-channel histogram data.
    pub channels: Vec<HistogramChannel>,
    /// Heuristic ratio used by KStars-style stretch logic.
    pub jm_index: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Low and high clipping points for one channel stretch.
pub struct StretchWindow {
    /// Intensity mapped to black.
    pub shadows: f64,
    /// Intensity mapped to white.
    pub highlights: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Stretch windows for all frame channels.
pub struct ImageStretch {
    /// Frame format this stretch was derived for.
    pub format: StandardImageFrameFormat,
    /// Per-channel stretch windows.
    pub channels: Vec<StretchWindow>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Midtones transfer parameters for one channel.
pub struct MidtonesTransfer {
    /// Intensity treated as black point before curve.
    pub shadows: f64,
    /// Midtones balance parameter in `(0, 1)`.
    pub midtones: f64,
    /// Intensity treated as white point before curve.
    pub highlights: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Output settings for rendered histogram preview.
pub struct HistogramRenderOptions {
    /// Output image width in pixels.
    pub width: u32,
    /// Output image height in pixels.
    pub height: u32,
    /// Use logarithmic bar scaling when true.
    pub logarithmic: bool,
}

impl Default for HistogramRenderOptions {
    /// Returns compact logarithmic histogram preview defaults.
    fn default() -> Self {
        Self {
            width: 256,
            height: 128,
            logarithmic: true,
        }
    }
}

/// Computes per-channel statistics for raw frame data.
pub fn compute_raw_image_stats(frame: &ImageFrame) -> VastResult<RawImageStats> {
    validate_frame_len(frame)?;

    let channels = channel_count(frame.format);
    let pixels_per_channel = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .ok_or_else(|| file_error("image dimensions overflow".to_string()))?;

    let samples = planar_channel_samples(frame)?;
    let mut channel_stats = Vec::with_capacity(channels);

    for channel in samples {
        channel_stats.push(compute_channel_stats(channel)?);
    }

    Ok(RawImageStats {
        channels,
        pixels_per_channel,
        channel_stats,
    })
}

/// Builds per-channel histogram data from frame samples.
pub fn build_raw_histogram(frame: &ImageFrame, requested_bin_count: Option<usize>) -> VastResult<RawHistogram> {
    let stats = compute_raw_image_stats(frame)?;
    let samples = planar_channel_samples(frame)?;

    let mut max_range = 0.0_f64;
    for channel in &stats.channel_stats {
        max_range = max_range.max(channel.max - channel.min);
    }

    let mut bin_count = requested_bin_count.unwrap_or_else(|| {
        if max_range.is_finite() && max_range > 0.0 {
            max_range.min(256.0).round() as usize
        } else {
            256
        }
    });
    if bin_count == 0 {
        bin_count = 256;
    }

    let sample_by = if stats.pixels_per_channel > 500_000 {
        stats.pixels_per_channel / 500_000
    } else {
        1
    };

    let mut channels = Vec::with_capacity(stats.channels);
    for (index, channel) in samples.into_iter().enumerate() {
        let channel_stats = &stats.channel_stats[index];
        let min_bin_size: f64 = if channel_stats.max > 1.1 { 1.0 } else { 0.0001 };
        let bin_width = min_bin_size.max((channel_stats.max - channel_stats.min) / bin_count as f64);

        let mut intensities = vec![0.0; bin_count + 1];
        let mut frequencies = vec![0_u32; bin_count + 1];
        let mut cumulative_frequencies = vec![0_u32; bin_count + 1];

        for (i, intensity) in intensities.iter_mut().enumerate().take(bin_count) {
            *intensity = channel_stats.min + bin_width * i as f64;
        }

        for sample in channel.iter().step_by(sample_by) {
            let mut id = ((sample - channel_stats.min) / bin_width).round() as isize;
            id = id.clamp(0, bin_count as isize);
            frequencies[id as usize] = frequencies[id as usize].saturating_add(sample_by as u32);
        }

        let mut accumulator = 0_u32;
        for i in 0..=bin_count {
            accumulator = accumulator.saturating_add(frequencies[i]);
            cumulative_frequencies[i] = accumulator;
        }

        channels.push(HistogramChannel {
            intensities,
            frequencies,
            cumulative_frequencies,
            bin_width,
        });
    }

    let jm_index = if let Some(first) = channels.first() {
        let q4 = bin_count / 4;
        let q8 = bin_count / 8;
        let denominator = first.cumulative_frequencies.get(q4).copied().unwrap_or(0);
        if denominator > 0 {
            first.cumulative_frequencies.get(q8).copied().unwrap_or(0) as f64 / denominator as f64
        } else {
            1.0
        }
    } else {
        1.0
    };

    Ok(RawHistogram {
        bin_count,
        channels,
        jm_index,
    })
}

/// Estimates simple KStars-style auto-stretch windows from mean and standard deviation.
pub fn compute_auto_stretch(frame: &ImageFrame) -> VastResult<ImageStretch> {
    let stats = compute_raw_image_stats(frame)?;
    let mut channels = Vec::with_capacity(stats.channels);

    for channel in stats.channel_stats {
        let shadows = channel.mean - channel.stddev;
        let highlights = channel.mean + channel.stddev * 3.0;
        channels.push(clamp_stretch_window(frame.format, shadows, highlights));
    }

    Ok(ImageStretch {
        format: frame.format,
        channels,
    })
}

/// Estimates stretch windows from low and high per-channel percentiles.
pub fn compute_percentile_auto_stretch(
    frame: &ImageFrame,
    low_percentile: f64,
    high_percentile: f64,
) -> VastResult<ImageStretch> {
    validate_frame_len(frame)?;
    if !(0.0..=1.0).contains(&low_percentile)
        || !(0.0..=1.0).contains(&high_percentile)
        || low_percentile >= high_percentile
    {
        return Err(file_error(format!(
            "invalid percentile stretch window: low={low_percentile}, high={high_percentile}"
        )));
    }

    let samples = planar_channel_samples(frame)?;
    let mut channels = Vec::with_capacity(samples.len());
    for mut channel in samples {
        channel.sort_by(f64::total_cmp);
        let shadows = percentile_of_sorted(&channel, low_percentile);
        let highlights = percentile_of_sorted(&channel, high_percentile);
        channels.push(clamp_stretch_window(frame.format, shadows, highlights));
    }

    Ok(ImageStretch {
        format: frame.format,
        channels,
    })
}

/// Applies linear per-channel stretch windows to frame and returns stretched copy.
pub fn apply_stretch(frame: &ImageFrame, stretch: &ImageStretch) -> VastResult<ImageFrame> {
    validate_frame_len(frame)?;

    if stretch.format != frame.format {
        return Err(file_error(format!(
            "stretch format {} does not match frame format {}",
            stretch.format.name(),
            frame.format.name()
        )));
    }

    let channels = channel_count(frame.format);
    if stretch.channels.len() != channels {
        return Err(file_error(format!(
            "stretch channel count {} does not match frame channel count {}",
            stretch.channels.len(),
            channels
        )));
    }

    let pixels_per_channel = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .ok_or_else(|| file_error("image dimensions overflow".to_string()))?;
    let max_value = format_max_value(frame.format);
    let samples = planar_channel_samples(frame)?;

    let mut data = Vec::with_capacity(frame.data.len());
    for (channel_index, channel) in samples.into_iter().enumerate() {
        let window = stretch.channels[channel_index];
        let range = (window.highlights - window.shadows).max(f64::EPSILON);

        match frame.format {
            StandardImageFrameFormat::RAW8 => {
                for sample in channel {
                    let stretched = ((sample - window.shadows) / range).clamp(0.0, 1.0);
                    data.push((stretched * max_value).round() as u8);
                }
            }
            StandardImageFrameFormat::RAW10
            | StandardImageFrameFormat::RAW12
            | StandardImageFrameFormat::RAW14
            | StandardImageFrameFormat::RAW16 => {
                for sample in channel {
                    let stretched = ((sample - window.shadows) / range).clamp(0.0, 1.0);
                    let sample_value = (stretched * max_value).round() as u16;
                    data.extend_from_slice(&sample_value.to_ne_bytes());
                }
            }
            StandardImageFrameFormat::RGB24 => {
                for sample in channel.into_iter().take(pixels_per_channel) {
                    let stretched = ((sample - window.shadows) / range).clamp(0.0, 1.0);
                    data.push((stretched * max_value).round() as u8);
                }
            }
            StandardImageFrameFormat::RGB32 => {
                for sample in channel.into_iter().take(pixels_per_channel) {
                    let stretched = ((sample - window.shadows) / range).clamp(0.0, 1.0);
                    data.push((stretched * max_value).round() as u8);
                }
            }
        }
    }

    Ok(ImageFrame {
        width: frame.width,
        height: frame.height,
        format: frame.format,
        data,
    })
}

/// Applies PixInsight-style midtones transfer curve to each frame channel.
pub fn apply_midtones_transfer(
    frame: &ImageFrame,
    transfer: &[MidtonesTransfer],
) -> VastResult<ImageFrame> {
    validate_frame_len(frame)?;

    let channels = channel_count(frame.format);
    if transfer.len() != channels {
        return Err(file_error(format!(
            "midtones transfer channel count {} does not match frame channel count {}",
            transfer.len(),
            channels
        )));
    }

    let max_value = format_max_value(frame.format);
    let samples = planar_channel_samples(frame)?;
    let mut data = Vec::with_capacity(frame.data.len());

    for (channel_index, channel) in samples.into_iter().enumerate() {
        let params = transfer[channel_index];
        if !(0.0..=1.0).contains(&params.midtones) || params.midtones == 0.0 || params.midtones == 1.0 {
            return Err(file_error(format!(
                "invalid midtones parameter {} for channel {}",
                params.midtones, channel_index
            )));
        }

        let shadows = params.shadows;
        let highlights = params.highlights.max(shadows + f64::EPSILON);
        let range = highlights - shadows;

        match frame.format {
            StandardImageFrameFormat::RAW8 => {
                for sample in channel {
                    let normalized = ((sample - shadows) / range).clamp(0.0, 1.0);
                    let transformed = midtones_curve(normalized, params.midtones);
                    data.push((transformed * max_value).round() as u8);
                }
            }
            StandardImageFrameFormat::RAW10
            | StandardImageFrameFormat::RAW12
            | StandardImageFrameFormat::RAW14
            | StandardImageFrameFormat::RAW16 => {
                for sample in channel {
                    let normalized = ((sample - shadows) / range).clamp(0.0, 1.0);
                    let transformed = midtones_curve(normalized, params.midtones);
                    let sample_value = (transformed * max_value).round() as u16;
                    data.extend_from_slice(&sample_value.to_ne_bytes());
                }
            }
            StandardImageFrameFormat::RGB24 | StandardImageFrameFormat::RGB32 => {
                for sample in channel {
                    let normalized = ((sample - shadows) / range).clamp(0.0, 1.0);
                    let transformed = midtones_curve(normalized, params.midtones);
                    data.push((transformed * max_value).round() as u8);
                }
            }
        }
    }

    Ok(ImageFrame {
        width: frame.width,
        height: frame.height,
        format: frame.format,
        data,
    })
}

/// Renders histogram frequencies into planar `RGB24` preview image.
pub fn render_histogram_visualization(
    histogram: &RawHistogram,
    options: HistogramRenderOptions,
) -> VastResult<ImageFrame> {
    if options.width == 0 || options.height == 0 {
        return Err(file_error("histogram visualization dimensions must be non-zero".to_string()));
    }
    if histogram.channels.is_empty() {
        return Err(file_error("cannot render empty histogram".to_string()));
    }

    let width = options.width as usize;
    let height = options.height as usize;
    let pixels_per_channel = width * height;
    let mut red = vec![0_u8; pixels_per_channel];
    let mut green = vec![0_u8; pixels_per_channel];
    let mut blue = vec![0_u8; pixels_per_channel];

    let peak = histogram
        .channels
        .iter()
        .flat_map(|channel| channel.frequencies.iter())
        .copied()
        .max()
        .unwrap_or(1) as f64;
    let log_peak = if options.logarithmic { (peak + 1.0).ln() } else { peak };

    for x in 0..width {
        let bin = x * histogram.bin_count / width;
        for (channel_index, channel) in histogram.channels.iter().enumerate().take(3) {
            let value = channel.frequencies.get(bin).copied().unwrap_or(0) as f64;
            let scaled = if options.logarithmic {
                ((value + 1.0).ln() / log_peak.max(f64::EPSILON)).clamp(0.0, 1.0)
            } else {
                (value / peak.max(f64::EPSILON)).clamp(0.0, 1.0)
            };
            let bar_height = (scaled * height as f64).round() as usize;

            for y in 0..bar_height.min(height) {
                let row = height - 1 - y;
                let index = row * width + x;
                match channel_index {
                    0 => red[index] = 255,
                    1 => green[index] = 255,
                    2 => blue[index] = 255,
                    _ => {}
                }
            }
        }
    }

    let mut data = Vec::with_capacity(pixels_per_channel * 3);
    data.extend_from_slice(&red);
    data.extend_from_slice(&green);
    data.extend_from_slice(&blue);

    Ok(ImageFrame {
        width: options.width,
        height: options.height,
        format: StandardImageFrameFormat::RGB24,
        data,
    })
}

/// Computes summary statistics for one channel sample buffer.
fn compute_channel_stats(channel: Vec<f64>) -> VastResult<ImageChannelStats> {
    if channel.is_empty() {
        return Err(file_error("cannot compute stats for empty channel".to_string()));
    }

    let mut sorted = channel;
    sorted.sort_by(f64::total_cmp);

    let min = sorted[0];
    let max = *sorted.last().unwrap();
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let median = median_of_sorted(&sorted);
    let variance = sorted
        .iter()
        .map(|sample| {
            let delta = sample - mean;
            delta * delta
        })
        .sum::<f64>()
        / sorted.len() as f64;
    let stddev = variance.sqrt();

    let mut deviations = sorted.iter().map(|sample| (sample - median).abs()).collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    let mad = median_of_sorted(&deviations);

    Ok(ImageChannelStats {
        min,
        max,
        mean,
        median,
        stddev,
        mad,
    })
}

/// Returns median value from already-sorted samples.
fn median_of_sorted(sorted: &[f64]) -> f64 {
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[middle - 1] + sorted[middle]) * 0.5
    } else {
        sorted[middle]
    }
}

/// Returns interpolated percentile from already-sorted samples.
fn percentile_of_sorted(sorted: &[f64], percentile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let index = percentile.clamp(0.0, 1.0) * (sorted.len().saturating_sub(1)) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let fraction = index - lower as f64;
        sorted[lower] * (1.0 - fraction) + sorted[upper] * fraction
    }
}

/// Evaluates midtones transfer function on normalized input.
fn midtones_curve(value: f64, midtones: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    if value >= 1.0 {
        return 1.0;
    }

    let numerator = (midtones - 1.0) * value;
    let denominator = (2.0 * midtones - 1.0) * value - midtones;
    if denominator.abs() < f64::EPSILON {
        value
    } else {
        (numerator / denominator).clamp(0.0, 1.0)
    }
}

/// Decodes frame data into planar per-channel floating-point samples.
fn planar_channel_samples(frame: &ImageFrame) -> VastResult<Vec<Vec<f64>>> {
    let pixels_per_channel = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .ok_or_else(|| file_error("image dimensions overflow".to_string()))?;

    match frame.format {
        StandardImageFrameFormat::RAW8 => Ok(vec![frame.data.iter().map(|sample| f64::from(*sample)).collect()]),
        StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => Ok(vec![frame
            .data
            .chunks_exact(2)
            .map(|chunk| f64::from(u16::from_ne_bytes([chunk[0], chunk[1]])))
            .collect()]),
        StandardImageFrameFormat::RGB24 => Ok((0..3)
            .map(|channel| {
                frame.data[channel * pixels_per_channel..(channel + 1) * pixels_per_channel]
                    .iter()
                    .map(|sample| f64::from(*sample))
                    .collect()
            })
            .collect()),
        StandardImageFrameFormat::RGB32 => Ok((0..4)
            .map(|channel| {
                frame.data[channel * pixels_per_channel..(channel + 1) * pixels_per_channel]
                    .iter()
                    .map(|sample| f64::from(*sample))
                    .collect()
            })
            .collect()),
    }
}

/// Verifies frame byte length matches dimensions and format.
fn validate_frame_len(frame: &ImageFrame) -> VastResult<()> {
    let expected = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .and_then(|pixels| pixels.checked_mul(frame.format.bytes_per_pixel()))
        .ok_or_else(|| file_error("image dimensions overflow".to_string()))?;

    if frame.data.len() != expected {
        return Err(file_error(format!(
            "invalid frame data length: got {}, expected {}",
            frame.data.len(),
            expected
        )));
    }

    Ok(())
}

/// Returns logical channel count for image format.
fn channel_count(format: StandardImageFrameFormat) -> usize {
    match format {
        StandardImageFrameFormat::RAW8
        | StandardImageFrameFormat::RAW10
        | StandardImageFrameFormat::RAW12
        | StandardImageFrameFormat::RAW14
        | StandardImageFrameFormat::RAW16 => 1,
        StandardImageFrameFormat::RGB24 => 3,
        StandardImageFrameFormat::RGB32 => 4,
    }
}

/// Returns maximum representable sample value for image format.
fn format_max_value(format: StandardImageFrameFormat) -> f64 {
    match format {
        StandardImageFrameFormat::RAW8 | StandardImageFrameFormat::RGB24 | StandardImageFrameFormat::RGB32 => 255.0,
        StandardImageFrameFormat::RAW10 => 1023.0,
        StandardImageFrameFormat::RAW12 => 4095.0,
        StandardImageFrameFormat::RAW14 => 16383.0,
        StandardImageFrameFormat::RAW16 => 65535.0,
    }
}

/// Clamps stretch window into format range and preserves non-zero width.
fn clamp_stretch_window(
    format: StandardImageFrameFormat,
    mut shadows: f64,
    mut highlights: f64,
) -> StretchWindow {
    let max_value = format_max_value(format);
    shadows = shadows.clamp(0.0, max_value);
    highlights = highlights.clamp(0.0, max_value);

    if highlights <= shadows {
        highlights = (shadows + 1.0).min(max_value.max(1.0));
    }

    StretchWindow {
        shadows,
        highlights,
    }
}

/// Builds file-domain error used by image processing helpers.
fn file_error(message: String) -> VastError {
    VastError::new(VastErrorType::FileError, message)
}

use crate::algos::images::ThresholdMethod;

#[derive(Debug, Clone, Copy, PartialEq)]
/// Blob detection parameters for thresholded star finding.
pub struct StarDetectionOptions {
    /// Channel index to inspect.
    pub channel_index: usize,
    /// Thresholding heuristic used to separate stars from background.
    pub threshold_method: ThresholdMethod,
    /// Minimum connected pixels required for accepted blob.
    pub min_pixels: usize,
    /// Optional maximum connected pixels allowed for accepted blob.
    pub max_pixels: Option<usize>,
    /// Reject blobs touching this border margin.
    pub border_margin: u32,
}

impl Default for StarDetectionOptions {
    fn default() -> Self {
        Self {
            channel_index: 0,
            threshold_method: ThresholdMethod::MeanStdDev { sigma: 3.0 },
            min_pixels: 4,
            max_pixels: None,
            border_margin: 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Guide-oriented candidate extraction parameters inspired by PHD2 AutoFind.
pub struct StarCandidateOptions {
    /// Channel index to inspect.
    pub channel_index: usize,
    /// Downsample factor applied before PSF search.
    pub downsample: usize,
    /// Minimum normalized peak strength above local mean.
    pub threshold_sigma: f64,
    /// Radius used for local-maximum suppression.
    pub local_max_radius: u32,
    /// Radius used for local background mean estimate on convolved image.
    pub local_background_radius: u32,
    /// Merge candidates closer than this many pixels.
    pub merge_distance: f64,
    /// Reject candidates touching this border margin.
    pub border_margin: u32,
    /// Maximum number of candidates returned.
    pub max_candidates: usize,
    /// Apply 3x3 median hot-pixel suppression before PSF search.
    pub median_filter: bool,
}

impl Default for StarCandidateOptions {
    fn default() -> Self {
        Self {
            channel_index: 0,
            downsample: 1,
            threshold_sigma: 0.1,
            local_max_radius: 4,
            local_background_radius: 7,
            merge_distance: 5.0,
            border_margin: 8,
            max_candidates: 100,
            median_filter: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Local PSF-convolution maximum candidate for guider acquisition.
pub struct StarCandidate {
    /// Channel index used for extraction.
    pub channel_index: usize,
    /// Candidate X coordinate in original image pixels.
    pub x: f64,
    /// Candidate Y coordinate in original image pixels.
    pub y: f64,
    /// Candidate strength score from PSF response.
    pub score: f64,
    /// Peak value on convolved image.
    pub peak_value: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Thresholded connected component that may correspond to a star.
pub struct StarBlob {
    /// Channel index blob was detected in.
    pub channel_index: usize,
    /// Threshold level used for detection.
    pub threshold: f64,
    /// Blob pixels in image coordinates.
    pub pixels: Vec<(u32, u32)>,
    /// Inclusive left bound.
    pub min_x: u32,
    /// Inclusive top bound.
    pub min_y: u32,
    /// Inclusive right bound.
    pub max_x: u32,
    /// Inclusive bottom bound.
    pub max_y: u32,
    /// Peak pixel X coordinate.
    pub peak_x: u32,
    /// Peak pixel Y coordinate.
    pub peak_y: u32,
    /// Peak sample value.
    pub peak_value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Background-subtracted centroid for one detected blob.
pub struct StarCentroid {
    /// Centroid X coordinate in pixel-center space.
    pub x: f64,
    /// Centroid Y coordinate in pixel-center space.
    pub y: f64,
    /// Sum of background-subtracted weights.
    pub total_mass: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Local background estimate around one blob.
pub struct StarBackground {
    /// Mean background level in annulus.
    pub mean: f64,
    /// Background standard deviation.
    pub sigma: f64,
    /// Inner annulus radius in pixels.
    pub inner_radius: f64,
    /// Outer annulus radius in pixels.
    pub outer_radius: f64,
    /// Number of pixels used in estimate.
    pub pixels: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Flux-radius metrics for one detected blob.
pub struct StarShapeMetrics {
    /// Half-flux radius in pixels.
    pub hfr: f64,
    /// Half-flux diameter in pixels.
    pub hfd: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Guide-star ranking parameters inspired by KStars guide star selection.
pub struct StarScoringOptions {
    /// Reject stars with HFR above this value when available.
    pub max_hfr: Option<f64>,
    /// Penalize stars closer than this many pixels to image border.
    pub border_guard: u32,
    /// Ideal SNR lower bound for bonus.
    pub preferred_snr_min: f64,
    /// Ideal SNR upper bound for bonus.
    pub preferred_snr_max: f64,
    /// Penalize SNR above this value slightly.
    pub oversaturated_snr: f64,
    /// Penalize neighbors closer than this many pixels.
    pub close_neighbor_distance: f64,
    /// Penalize heavily neighbors closer than this many pixels.
    pub very_close_neighbor_distance: f64,
}

impl Default for StarScoringOptions {
    fn default() -> Self {
        Self {
            max_hfr: Some(16.0),
            border_guard: 35,
            preferred_snr_min: 40.0,
            preferred_snr_max: 100.0,
            oversaturated_snr: 100.0,
            close_neighbor_distance: 25.0,
            very_close_neighbor_distance: 15.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Derived guide-star score and measurements for one blob.
pub struct StarScore {
    /// Zero-based blob index in input slice.
    pub index: usize,
    /// Final score, higher is better.
    pub score: f64,
    /// Blob signal-to-noise estimate.
    pub snr: f64,
    /// Half-flux radius in pixels.
    pub hfr: f64,
    /// Distance to nearest other blob centroid in pixels.
    pub nearest_neighbor_distance: f64,
}

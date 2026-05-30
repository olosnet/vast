//! Star/blob detection and centroid helpers.
//!
//! References:
//! - KStars `kstars/fitsviewer/fitsthresholddetector.cpp`
//! - KStars `kstars/fitsviewer/fitscentroiddetector.cpp`
//! - KStars `kstars/ekos/guide/internalguide/guidestars.cpp`
//! - PHD2 `src/star.cpp`
//! - PHD2 `src/image_math.cpp`
//!
//! Implementation here stays simpler than those projects: it uses thresholded
//! connected components and background-subtracted center-of-mass centroiding.

use crate::{
    base::errors::{VastError, VastErrorType, VastResult},
    types::imageformats::ImageFrame,
};

use super::images::{compute_raw_image_stats, compute_threshold, extract_channel_samples};

pub use crate::types::stars::{
    StarBackground, StarBlob, StarCandidate, StarCandidateOptions, StarCentroid,
    StarDetectionOptions, StarScore, StarScoringOptions, StarShapeMetrics,
};

/// Applies 3x3 median filtering to one image channel to suppress isolated hot pixels.
pub fn median_filter_channel_3x3(frame: &ImageFrame, channel_index: usize) -> VastResult<Vec<f64>> {
    let stats = compute_raw_image_stats(frame)?;
    if channel_index >= stats.channels {
        return Err(file_error(format!(
            "median filter channel index {} out of range for {} channels",
            channel_index, stats.channels
        )));
    }

    let samples = extract_channel_samples(frame, channel_index)?;
    let width = frame.width as usize;
    let height = frame.height as usize;
    if width < 3 || height < 3 {
        return Ok(samples);
    }

    let mut filtered = samples.clone();
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let mut window = [0.0_f64; 9];
            let mut index = 0;
            for ny in y - 1..=y + 1 {
                for nx in x - 1..=x + 1 {
                    window[index] = samples[ny * width + nx];
                    index += 1;
                }
            }
            window.sort_by(f64::total_cmp);
            filtered[y * width + x] = window[4];
        }
    }

    Ok(filtered)
}

/// Finds local PSF-convolution star candidates for future internal-guider acquisition.
pub fn find_star_candidates(frame: &ImageFrame, options: StarCandidateOptions) -> VastResult<Vec<StarCandidate>> {
    let stats = compute_raw_image_stats(frame)?;
    if options.channel_index >= stats.channels {
        return Err(file_error(format!(
            "star candidate channel index {} out of range for {} channels",
            options.channel_index, stats.channels
        )));
    }
    if options.downsample == 0 {
        return Err(file_error("star candidate downsample must be greater than zero".to_string()));
    }
    if options.max_candidates == 0 {
        return Err(file_error("star candidate max_candidates must be greater than zero".to_string()));
    }
    if !options.threshold_sigma.is_finite() {
        return Err(file_error("star candidate threshold_sigma must be finite".to_string()));
    }

    let samples = if options.median_filter {
        median_filter_channel_3x3(frame, options.channel_index)?
    } else {
        extract_channel_samples(frame, options.channel_index)?
    };
    let (samples, width, height) = downsample_average(&samples, frame.width as usize, frame.height as usize, options.downsample);
    let psf_radius = 4_usize;
    if width <= psf_radius * 2 || height <= psf_radius * 2 {
        return Ok(Vec::new());
    }

    let convolved = psf_convolution(&samples, width, height);
    let valid_min = psf_radius.max(options.local_max_radius as usize);
    let valid_max_x = width.saturating_sub(valid_min);
    let valid_max_y = height.saturating_sub(valid_min);
    if valid_min >= valid_max_x || valid_min >= valid_max_y {
        return Ok(Vec::new());
    }

    let (global_mean, global_stddev) = compute_region_stats(&convolved, width, valid_min, valid_min, valid_max_x, valid_max_y)?;
    let global_stddev = global_stddev.max(f64::EPSILON);
    let mut candidates = Vec::new();

    for y in valid_min..valid_max_y {
        for x in valid_min..valid_max_x {
            let value = convolved[y * width + x];
            if value <= 0.0 {
                continue;
            }
            if !is_local_maximum(&convolved, width, height, x, y, options.local_max_radius as usize) {
                continue;
            }

            let local_radius = options.local_background_radius as usize;
            let local_min_x = x.saturating_sub(local_radius).max(psf_radius);
            let local_min_y = y.saturating_sub(local_radius).max(psf_radius);
            let local_max_x = (x + local_radius + 1).min(width - psf_radius);
            let local_max_y = (y + local_radius + 1).min(height - psf_radius);
            let (local_mean, _) = compute_region_stats(&convolved, width, local_min_x, local_min_y, local_max_x, local_max_y)?;
            let score = (value - local_mean).max(0.0) / global_stddev;
            if score < options.threshold_sigma {
                continue;
            }

            let original_x = x as f64 * options.downsample as f64 + options.downsample as f64 * 0.5;
            let original_y = y as f64 * options.downsample as f64 + options.downsample as f64 * 0.5;
            if original_x < options.border_margin as f64
                || original_y < options.border_margin as f64
                || original_x > frame.width as f64 - options.border_margin as f64
                || original_y > frame.height as f64 - options.border_margin as f64
            {
                continue;
            }

            candidates.push(StarCandidate {
                channel_index: options.channel_index,
                x: original_x,
                y: original_y,
                score,
                peak_value: value.max(global_mean),
            });
        }
    }

    candidates.sort_by(|lhs, rhs| rhs.score.total_cmp(&lhs.score));
    let mut merged = Vec::new();
    for candidate in candidates {
        if merged.iter().any(|existing: &StarCandidate| {
            ((candidate.x - existing.x).powi(2) + (candidate.y - existing.y).powi(2)).sqrt() < options.merge_distance
        }) {
            continue;
        }
        merged.push(candidate);
        if merged.len() >= options.max_candidates {
            break;
        }
    }

    Ok(merged)
}

/// Detects thresholded star blobs using 8-connected component labeling.
pub fn detect_star_blobs(frame: &ImageFrame, options: StarDetectionOptions) -> VastResult<Vec<StarBlob>> {
    let stats = compute_raw_image_stats(frame)?;
    if options.channel_index >= stats.channels {
        return Err(file_error(format!(
            "star detection channel index {} out of range for {} channels",
            options.channel_index, stats.channels
        )));
    }
    if options.min_pixels == 0 {
        return Err(file_error("star detection min_pixels must be greater than zero".to_string()));
    }

    let threshold = compute_threshold(frame, options.threshold_method)?;
    let threshold_level = threshold.channels[options.channel_index];
    let samples = extract_channel_samples(frame, options.channel_index)?;
    let width = frame.width as usize;
    let height = frame.height as usize;
    let mut visited = vec![false; samples.len()];
    let mut blobs = Vec::new();

    for y in 0..height {
        for x in 0..width {
            let start = y * width + x;
            if visited[start] || samples[start] < threshold_level {
                continue;
            }

            let mut stack = vec![(x, y)];
            let mut pixels = Vec::new();
            let mut min_x = x as u32;
            let mut min_y = y as u32;
            let mut max_x = x as u32;
            let mut max_y = y as u32;
            let mut peak_x = x as u32;
            let mut peak_y = y as u32;
            let mut peak_value = samples[start];

            visited[start] = true;
            while let Some((cx, cy)) = stack.pop() {
                let index = cy * width + cx;
                let sample = samples[index];
                pixels.push((cx as u32, cy as u32));
                min_x = min_x.min(cx as u32);
                min_y = min_y.min(cy as u32);
                max_x = max_x.max(cx as u32);
                max_y = max_y.max(cy as u32);
                if sample > peak_value {
                    peak_value = sample;
                    peak_x = cx as u32;
                    peak_y = cy as u32;
                }

                let min_nx = cx.saturating_sub(1);
                let max_nx = (cx + 1).min(width - 1);
                let min_ny = cy.saturating_sub(1);
                let max_ny = (cy + 1).min(height - 1);
                for ny in min_ny..=max_ny {
                    for nx in min_nx..=max_nx {
                        let neighbor = ny * width + nx;
                        if visited[neighbor] || samples[neighbor] < threshold_level {
                            continue;
                        }
                        visited[neighbor] = true;
                        stack.push((nx, ny));
                    }
                }
            }

            if pixels.len() < options.min_pixels {
                continue;
            }
            if options.max_pixels.is_some_and(|max_pixels| pixels.len() > max_pixels) {
                continue;
            }
            if touches_border(min_x, min_y, max_x, max_y, frame.width, frame.height, options.border_margin) {
                continue;
            }

            blobs.push(StarBlob {
                channel_index: options.channel_index,
                threshold: threshold_level,
                pixels,
                min_x,
                min_y,
                max_x,
                max_y,
                peak_x,
                peak_y,
                peak_value,
            });
        }
    }

    blobs.sort_by(|lhs, rhs| rhs.peak_value.total_cmp(&lhs.peak_value));
    Ok(blobs)
}

/// Computes background-subtracted center-of-mass centroid for one blob.
pub fn compute_blob_centroid(frame: &ImageFrame, blob: &StarBlob) -> VastResult<StarCentroid> {
    let (inner_radius, outer_radius) = default_background_radii(frame, blob);
    let background = estimate_blob_background(frame, blob, inner_radius, outer_radius)?;
    compute_blob_centroid_with_background(frame, blob, background)
}

/// Computes local annulus background around one blob.
pub fn estimate_blob_background(
    frame: &ImageFrame,
    blob: &StarBlob,
    inner_radius: f64,
    outer_radius: f64,
) -> VastResult<StarBackground> {
    let stats = compute_raw_image_stats(frame)?;
    if blob.channel_index >= stats.channels {
        return Err(file_error(format!(
            "blob channel index {} out of range for {} channels",
            blob.channel_index, stats.channels
        )));
    }
    if !inner_radius.is_finite() || !outer_radius.is_finite() || inner_radius < 0.0 || outer_radius <= inner_radius {
        return Err(file_error("invalid annulus radii".to_string()));
    }

    let samples = extract_channel_samples(frame, blob.channel_index)?;
    let width = frame.width as usize;
    let height = frame.height as usize;
    let center_x = blob.peak_x as f64 + 0.5;
    let center_y = blob.peak_y as f64 + 0.5;
    let inner2 = inner_radius * inner_radius;
    let outer2 = outer_radius * outer_radius;
    let start_x = blob.peak_x.saturating_sub(outer_radius.ceil() as u32) as usize;
    let start_y = blob.peak_y.saturating_sub(outer_radius.ceil() as u32) as usize;
    let end_x = ((blob.peak_x + outer_radius.ceil() as u32 + 1).min(frame.width)) as usize;
    let end_y = ((blob.peak_y + outer_radius.ceil() as u32 + 1).min(frame.height)) as usize;
    let mut values = Vec::new();

    for y in start_y..end_y.min(height) {
        for x in start_x..end_x.min(width) {
            let dx = x as f64 + 0.5 - center_x;
            let dy = y as f64 + 0.5 - center_y;
            let distance2 = dx * dx + dy * dy;
            if distance2 <= inner2 || distance2 > outer2 {
                continue;
            }
            values.push(samples[y * width + x]);
        }
    }

    if values.is_empty() {
        return Err(file_error("cannot estimate star background from empty annulus".to_string()));
    }

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let sigma = (values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64)
        .sqrt();

    Ok(StarBackground {
        mean,
        sigma,
        inner_radius,
        outer_radius,
        pixels: values.len(),
    })
}

/// Computes background-subtracted center-of-mass centroid using caller-provided background.
pub fn compute_blob_centroid_with_background(
    frame: &ImageFrame,
    blob: &StarBlob,
    background: StarBackground,
) -> VastResult<StarCentroid> {
    let stats = compute_raw_image_stats(frame)?;
    if blob.channel_index >= stats.channels {
        return Err(file_error(format!(
            "blob channel index {} out of range for {} channels",
            blob.channel_index, stats.channels
        )));
    }

    let samples = extract_channel_samples(frame, blob.channel_index)?;
    let width = frame.width as usize;
    let mut total_mass = 0.0;
    let mut weighted_x = 0.0;
    let mut weighted_y = 0.0;

    for (x, y) in &blob.pixels {
        let index = (*y as usize)
            .checked_mul(width)
            .and_then(|row| row.checked_add(*x as usize))
            .ok_or_else(|| file_error("blob pixel index overflow".to_string()))?;
        let weight = (samples[index] - background.mean).max(0.0);
        if weight <= 0.0 {
            continue;
        }
        total_mass += weight;
        weighted_x += (*x as f64 + 0.5) * weight;
        weighted_y += (*y as f64 + 0.5) * weight;
    }

    if total_mass <= f64::EPSILON {
        return Err(file_error("cannot compute centroid for empty blob mass".to_string()));
    }

    Ok(StarCentroid {
        x: weighted_x / total_mass,
        y: weighted_y / total_mass,
        total_mass,
    })
}

/// Computes half-flux metrics for one blob using centroid and background.
pub fn compute_blob_shape_metrics(
    frame: &ImageFrame,
    blob: &StarBlob,
    centroid: StarCentroid,
    background: StarBackground,
) -> VastResult<StarShapeMetrics> {
    let samples = extract_channel_samples(frame, blob.channel_index)?;
    let width = frame.width as usize;
    let mut radial_mass = Vec::new();
    let mut total_mass = 0.0;

    for (x, y) in &blob.pixels {
        let index = (*y as usize)
            .checked_mul(width)
            .and_then(|row| row.checked_add(*x as usize))
            .ok_or_else(|| file_error("blob pixel index overflow".to_string()))?;
        let mass = (samples[index] - background.mean).max(0.0);
        if mass <= 0.0 {
            continue;
        }
        let dx = *x as f64 + 0.5 - centroid.x;
        let dy = *y as f64 + 0.5 - centroid.y;
        radial_mass.push((dx * dx + dy * dy, mass));
        total_mass += mass;
    }

    if radial_mass.is_empty() || total_mass <= f64::EPSILON {
        return Err(file_error("cannot compute HFR for empty blob mass".to_string()));
    }
    if radial_mass.len() == 1 {
        return Ok(StarShapeMetrics { hfr: 0.25, hfd: 0.5 });
    }

    radial_mass.sort_by(|lhs, rhs| lhs.0.total_cmp(&rhs.0));
    let half_mass = total_mass * 0.5;
    let mut previous_radius2 = 0.0_f64;
    let mut previous_mass = 0.0_f64;
    let mut cumulative_mass = 0.0_f64;
    let mut hfr = 0.25;

    for (radius2, mass) in radial_mass {
        cumulative_mass += mass;
        if cumulative_mass >= half_mass {
            let radius0 = previous_radius2.sqrt();
            let radius1 = radius2.sqrt();
            if cumulative_mass > previous_mass {
                hfr = radius0 + (radius1 - radius0) * (half_mass - previous_mass) / (cumulative_mass - previous_mass);
            } else {
                hfr = radius1;
            }
            break;
        }
        previous_radius2 = radius2;
        previous_mass = cumulative_mass;
    }

    Ok(StarShapeMetrics { hfr, hfd: hfr * 2.0 })
}

/// Scores detected stars for guide-star selection.
pub fn score_star_blobs(
    frame: &ImageFrame,
    blobs: &[StarBlob],
    options: StarScoringOptions,
) -> VastResult<Vec<StarScore>> {
    let mut scored = Vec::with_capacity(blobs.len());
    let mut centroids = Vec::with_capacity(blobs.len());

    for blob in blobs {
        let (inner_radius, outer_radius) = default_background_radii(frame, blob);
        let background = estimate_blob_background(frame, blob, inner_radius, outer_radius)?;
        let centroid = compute_blob_centroid_with_background(frame, blob, background)?;
        let shape = compute_blob_shape_metrics(frame, blob, centroid, background)?;
        centroids.push((centroid, background, shape));
    }

    for (index, blob) in blobs.iter().enumerate() {
        let (centroid, background, shape) = centroids[index];
        let snr = centroid.total_mass
            / (centroid.total_mass + background.sigma * background.sigma * blob.pixels.len() as f64)
                .sqrt()
                .max(f64::EPSILON);
        let nearest_neighbor_distance = centroids
            .iter()
            .enumerate()
            .filter(|(other_index, _)| *other_index != index)
            .map(|(_, (other, _, _))| ((centroid.x - other.x).powi(2) + (centroid.y - other.y).powi(2)).sqrt())
            .min_by(f64::total_cmp)
            .unwrap_or(f64::INFINITY);

        let mut score = 100.0 + snr;
        if blob.min_x < options.border_guard
            || blob.min_y < options.border_guard
            || blob.max_x.saturating_add(options.border_guard) >= frame.width
            || blob.max_y.saturating_add(options.border_guard) >= frame.height
        {
            score -= 1000.0;
        }
        if options.max_hfr.is_some_and(|max_hfr| shape.hfr > max_hfr) {
            score -= 1000.0;
        }
        if snr >= options.preferred_snr_min && snr <= options.preferred_snr_max {
            score += 75.0;
        }
        if snr >= options.oversaturated_snr {
            score -= 50.0;
        }
        if nearest_neighbor_distance < options.very_close_neighbor_distance {
            score -= 100.0;
        } else if nearest_neighbor_distance < options.close_neighbor_distance {
            score -= 50.0;
        }

        scored.push(StarScore {
            index,
            score,
            snr,
            hfr: shape.hfr,
            nearest_neighbor_distance,
        });
    }

    scored.sort_by(|lhs, rhs| rhs.score.total_cmp(&lhs.score));
    Ok(scored)
}

fn default_background_radii(frame: &ImageFrame, blob: &StarBlob) -> (f64, f64) {
    let center_x = blob.peak_x as f64 + 0.5;
    let center_y = blob.peak_y as f64 + 0.5;
    let max_radius = center_x
        .min(center_y)
        .min(frame.width as f64 - center_x)
        .min(frame.height as f64 - center_y)
        .max(1.5);

    let outer_radius = max_radius.min(12.0);
    let inner_radius = (outer_radius * 0.5).max(1.0).min((outer_radius - 0.5).max(0.0));
    (inner_radius, outer_radius)
}

fn touches_border(
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    width: u32,
    height: u32,
    border_margin: u32,
) -> bool {
    min_x < border_margin
        || min_y < border_margin
        || max_x.saturating_add(border_margin) >= width
        || max_y.saturating_add(border_margin) >= height
}

fn downsample_average(samples: &[f64], width: usize, height: usize, factor: usize) -> (Vec<f64>, usize, usize) {
    if factor <= 1 {
        return (samples.to_vec(), width, height);
    }

    let down_width = width / factor;
    let down_height = height / factor;
    if down_width == 0 || down_height == 0 {
        return (samples.to_vec(), width, height);
    }

    let mut output = vec![0.0; down_width * down_height];
    for y in 0..down_height {
        for x in 0..down_width {
            let mut sum = 0.0;
            for ny in 0..factor {
                for nx in 0..factor {
                    sum += samples[(y * factor + ny) * width + (x * factor + nx)];
                }
            }
            output[y * down_width + x] = sum / (factor * factor) as f64;
        }
    }
    (output, down_width, down_height)
}

fn psf_convolution(samples: &[f64], width: usize, height: usize) -> Vec<f64> {
    let mut output = vec![0.0; samples.len()];
    if width < 9 || height < 9 {
        return output;
    }

    const A: f64 = 0.906;
    const B1: f64 = 0.584;
    const B2: f64 = 0.365;
    const C1: f64 = 0.117;
    const C2: f64 = 0.049;
    const C3: f64 = -0.05;
    const D1: f64 = -0.064;
    const D2: f64 = -0.074;
    const D3: f64 = -0.094;

    for y in 4..height - 4 {
        for x in 4..width - 4 {
            let a = samples[y * width + x];
            let b1 = samples[(y - 1) * width + x]
                + samples[(y + 1) * width + x]
                + samples[y * width + x - 1]
                + samples[y * width + x + 1];
            let b2 = samples[(y - 1) * width + x - 1]
                + samples[(y - 1) * width + x + 1]
                + samples[(y + 1) * width + x - 1]
                + samples[(y + 1) * width + x + 1];
            let c1 = samples[(y - 2) * width + x]
                + samples[(y + 2) * width + x]
                + samples[y * width + x - 2]
                + samples[y * width + x + 2];
            let c2 = samples[(y - 2) * width + x - 1]
                + samples[(y - 2) * width + x + 1]
                + samples[(y - 1) * width + x - 2]
                + samples[(y - 1) * width + x + 2]
                + samples[(y + 1) * width + x - 2]
                + samples[(y + 1) * width + x + 2]
                + samples[(y + 2) * width + x - 1]
                + samples[(y + 2) * width + x + 1];
            let c3 = samples[(y - 2) * width + x - 2]
                + samples[(y - 2) * width + x + 2]
                + samples[(y + 2) * width + x - 2]
                + samples[(y + 2) * width + x + 2];
            let d1 = samples[(y - 3) * width + x]
                + samples[(y + 3) * width + x]
                + samples[y * width + x - 3]
                + samples[y * width + x + 3];
            let d2 = samples[(y - 3) * width + x - 1]
                + samples[(y - 3) * width + x + 1]
                + samples[(y - 1) * width + x - 3]
                + samples[(y - 1) * width + x + 3]
                + samples[(y + 1) * width + x - 3]
                + samples[(y + 1) * width + x + 3]
                + samples[(y + 3) * width + x - 1]
                + samples[(y + 3) * width + x + 1];

            let mut d3 = 0.0;
            for py in y - 4..=y + 4 {
                for px in x - 4..=x + 4 {
                    let dx = px.abs_diff(x);
                    let dy = py.abs_diff(y);
                    if dx <= 3 && dy <= 3 {
                        continue;
                    }
                    d3 += samples[py * width + px];
                }
            }

            let mean = (a + b1 + b2 + c1 + c2 + c3 + d1 + d2 + d3) / 81.0;
            output[y * width + x] = A * (a - mean)
                + B1 * (b1 - 4.0 * mean)
                + B2 * (b2 - 4.0 * mean)
                + C1 * (c1 - 4.0 * mean)
                + C2 * (c2 - 8.0 * mean)
                + C3 * (c3 - 4.0 * mean)
                + D1 * (d1 - 4.0 * mean)
                + D2 * (d2 - 8.0 * mean)
                + D3 * (d3 - 44.0 * mean);
        }
    }

    output
}

fn compute_region_stats(
    samples: &[f64],
    width: usize,
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
) -> VastResult<(f64, f64)> {
    let mut count = 0_usize;
    let mut mean = 0.0;
    let mut q = 0.0;

    for y in min_y..max_y {
        for x in min_x..max_x {
            let value = samples[y * width + x];
            count += 1;
            let delta = value - mean;
            mean += delta / count as f64;
            q += delta * (value - mean);
        }
    }

    if count == 0 {
        return Err(file_error("cannot compute region stats for empty region".to_string()));
    }

    Ok((mean, (q / count as f64).sqrt()))
}

fn is_local_maximum(samples: &[f64], width: usize, height: usize, x: usize, y: usize, radius: usize) -> bool {
    let value = samples[y * width + x];
    for ny in y.saturating_sub(radius)..=(y + radius).min(height - 1) {
        for nx in x.saturating_sub(radius)..=(x + radius).min(width - 1) {
            if nx == x && ny == y {
                continue;
            }
            if samples[ny * width + nx] > value {
                return false;
            }
        }
    }
    true
}

fn file_error(message: String) -> VastError {
    VastError::new(VastErrorType::FileError, message)
}

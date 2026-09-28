// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore androidx

//! Port of `SvgPathParser.kt` and `FeatureSerializer.kt` from androidx.graphics.shapes.

use super::ShapeError;
use super::cubic::Cubic;
use super::feature::{Feature, detect_features};
use super::rounded_polygon::{RoundedPolygon, fix_polygon_orientation};
use super::utils::{DISTANCE_EPSILON, Point};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// Converts each command (besides move to) of a svg path to a list of [Cubic]s.
/// Any svg path complying to the specification found at
/// https://www.w3.org/TR/SVG/paths.html is supported. Parameters can either be split
/// by whitespace or by commas.
///
/// Port of `SvgPathParser`. There is little error handling, as in the Kotlin
/// implementation: use with valid paths.
pub struct SvgPathParser;

impl SvgPathParser {
    /// Converts an SVG path string into a list of [Feature] objects. The polygon
    /// described in the path should represent a single, closed, non-self-intersecting
    /// polygon. Otherwise, either an error is returned or the
    /// [Morph](super::morph::Morph) that the polygon is used in will be distorted.
    ///
    /// Only the first shape within the SVG path is processed. Subsequent shapes or
    /// holes are ignored.
    ///
    /// Returns an error if the SVG path is invalid or represents a non-closed
    /// polygon.
    pub fn parse_features(svg_path: &str) -> Result<Vec<Feature>, ShapeError> {
        let parsed_cubics = Self::parse_cubics(svg_path)?;

        let continuous = |first: &Cubic, second: &Cubic| {
            (second.anchor0_x() - first.anchor1_x()).abs() < DISTANCE_EPSILON
                && (second.anchor0_y() - first.anchor1_y()).abs() < DISTANCE_EPSILON
        };

        let mut continuous_cubics_count = parsed_cubics.len();
        for index in 0..parsed_cubics.len().saturating_sub(1) {
            let current = &parsed_cubics[index];
            let next = &parsed_cubics[index + 1];
            if !continuous(current, next) {
                continuous_cubics_count = index;
                break;
            }
        }
        let first_shape_cubics = &parsed_cubics[..continuous_cubics_count];

        let parsed_polygon =
            RoundedPolygon::from_features(detect_features(first_shape_cubics), None)?;
        let fixed_polygon = fix_polygon_orientation(&parsed_polygon);

        Ok(fixed_polygon.features().to_vec())
    }

    /// Converts the path elements of `svg_path` to their cubic counterparts.
    /// `svg_path` corresponds to the data found in the path's `d` property.
    pub(crate) fn parse_cubics(svg_path: &str) -> Result<Vec<Cubic>, ShapeError> {
        // Regex (?=[mM]): split before each 'm' or 'M'.
        let paths = split_before(svg_path, |c| c == 'm' || c == 'M');
        let mut current = Point::ZERO;

        // The input may contain multiple move to commands, in which later ones can
        // be relative. Therefore we need to finish one path before we parse the
        // following, so we have the correct start positions
        let mut all_cubics = Vec::new();
        for path in paths.iter().filter(|p| !p.trim().is_empty()) {
            // Regex (?=[a-zA-Z]): split before each ASCII letter.
            let command_strings: Vec<String> = split_before(path, |c| c.is_ascii_alphabetic())
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .collect();
            if command_strings.is_empty() {
                continue;
            }

            // Paths start with move commands that define the starting position
            // Subsequent pairs are equal to line commands
            let move_to_command = Command::parse(&command_strings[0], current)?;
            current = move_to_command.start
                + Point { x: move_to_command.get(0), y: move_to_command.get(1) };

            let mut parser = ParserState::new(current);

            // Move to command already handled, handle subsequent line commands (if
            // any)
            parser.parse_command(&move_to_command.as_line(current))?;
            for cmd_str in command_strings.iter().skip(1) {
                let command = Command::parse(cmd_str, parser.position())?;
                parser.parse_command(&command)?;
            }

            all_cubics.extend(parser.cubics);
        }
        Ok(all_cubics)
    }
}

/// `String.split(Regex("(?=[mM])"))`-like: split `input` before each char matching
/// `pred`.
fn split_before(input: &str, pred: impl Fn(char) -> bool) -> Vec<String> {
    let mut result = Vec::new();
    let mut cur = String::new();
    for c in input.chars() {
        if pred(c) && !cur.is_empty() {
            result.push(core::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        result.push(cur);
    }
    result
}

struct ParserState {
    cubics: Vec<Cubic>,
    start: Point,
    previous_command: Option<Command>,
}

impl ParserState {
    fn new(start_position: Point) -> Self {
        Self { cubics: Vec::new(), start: start_position, previous_command: None }
    }

    fn position(&self) -> Point {
        self.cubics
            .last()
            .map(|c| Point { x: c.anchor1_x(), y: c.anchor1_y() })
            .unwrap_or(self.start)
    }

    fn reflected_previous_control_point(&self) -> Point {
        let position = self.position();
        let last = self.cubics.last().unwrap();
        position + (position - Point { x: last.control1_x(), y: last.control1_y() })
    }

    fn parse_command(&mut self, command: &Command) -> Result<(), ShapeError> {
        if command.is_close_command() {
            let position = self.position();
            self.cubics.push(Cubic::straight_line(
                position.x,
                position.y,
                self.start.x,
                self.start.y,
            ));
            return Ok(());
        }

        if command.params_count == 0 {
            return Ok(());
        }
        // A single SVG command can contain multiple parameter pairs. Split them into
        // atomics.
        let mut i = 0;
        while i + command.params_count <= command.parameters.len() {
            let atomic_command = command.chunk(i, self.position());
            self.parse_atomic_command(&atomic_command)?;
            i += command.params_count;
        }
        Ok(())
    }

    fn parse_atomic_command(&mut self, atomic_command: &Command) -> Result<(), ShapeError> {
        if atomic_command.is_line_command() {
            self.parse_line(atomic_command);
        } else if atomic_command.is_curve_command() {
            self.parse_curve(atomic_command);
        } else if atomic_command.is_arc_command() {
            self.parse_arc(atomic_command)?;
        }
        // Unknown commands are ignored (Kotlin logs a debug message).

        self.previous_command = Some(atomic_command.clone());
        Ok(())
    }

    fn parse_line(&mut self, command: &Command) {
        let position = self.position();
        let end_point = match command.letter {
            'l' => command.xy(0, 1),
            'h' => Point { x: command.x(0), y: command.start.y },
            'v' => Point { x: command.start.x, y: command.y(0) },
            _ => return,
        };
        self.cubics.push(Cubic::straight_line(position.x, position.y, end_point.x, end_point.y));
    }

    fn parse_curve(&mut self, command: &Command) {
        let position = self.position();
        match command.letter {
            'c' => self.cubics.push(Cubic::from_floats(
                position.x,
                position.y,
                command.xy(0, 1).x,
                command.xy(0, 1).y,
                command.xy(2, 3).x,
                command.xy(2, 3).y,
                command.xy(4, 5).x,
                command.xy(4, 5).y,
            )),
            's' => {
                let c0 = match &self.previous_command {
                    Some(prev) if prev.is_bezier_command() => {
                        self.reflected_previous_control_point()
                    }
                    _ => position,
                };
                let c1 = command.xy(0, 1);
                let a1 = command.xy(2, 3);
                self.cubics.push(Cubic::new(position, c0, c1, a1));
            }
            'q' => {
                let c0 = command.xy(0, 1);
                let a1 = command.xy(2, 3);
                self.cubics.push(Cubic::new(position, c0, c0, a1));
            }
            't' => {
                let c0 = match &self.previous_command {
                    Some(prev) if prev.is_quadratic_curve_command() => {
                        self.reflected_previous_control_point()
                    }
                    _ => position,
                };
                let a1 = command.xy(0, 1);
                self.cubics.push(Cubic::new(position, c0, c0, a1));
            }
            _ => {}
        }
    }

    fn parse_arc(&mut self, command: &Command) -> Result<(), ShapeError> {
        let target = command.xy(5, 6);
        let position = self.position();

        self.cubics.extend(arc_to_cubics(
            position.x,
            position.y,
            target.x,
            target.y,
            command.get(0),
            command.get(1),
            command.get(2),
            command.get(3) != 0.,
            command.get(4) != 0.,
        )?);
        Ok(())
    }
}

#[derive(Clone)]
struct Command {
    letter: char,
    is_relative: bool,
    parameters: Vec<f32>,
    params_count: usize,
    start: Point,
}

impl Command {
    fn parse(input: &str, current_position: Point) -> Result<Command, ShapeError> {
        let mut chars = input.chars();
        let letter_raw = chars.next().ok_or_else(|| ShapeError::new("Empty command"))?;
        let is_relative = letter_raw.is_ascii_lowercase();
        let rest: String = chars.collect();
        let parameters: Vec<f32> = rest
            .split([' ', ','])
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                s.trim().parse::<f32>().map_err(|_| ShapeError::new("Invalid float in SVG path"))
            })
            .collect::<Result<_, _>>()?;
        let letter = letter_raw.to_ascii_lowercase();
        let params_count = command_to_params_count(letter);
        Ok(Command {
            letter,
            is_relative,
            parameters,
            params_count,
            start: if is_relative { current_position } else { Point::ZERO },
        })
    }

    fn get(&self, i: usize) -> f32 {
        self.parameters[i]
    }

    fn x(&self, i: usize) -> f32 {
        let coordinate = self.get(i);
        if self.is_relative { self.start.x + coordinate } else { coordinate }
    }

    fn y(&self, i: usize) -> f32 {
        let coordinate = self.get(i);
        if self.is_relative { self.start.y + coordinate } else { coordinate }
    }

    fn xy(&self, i: usize, j: usize) -> Point {
        let coordinates = Point { x: self.get(i), y: self.get(j) };
        if self.is_relative { self.start + coordinates } else { coordinates }
    }

    fn is_line_command(&self) -> bool {
        matches!(self.letter, 'l' | 'h' | 'v')
    }
    fn is_bezier_command(&self) -> bool {
        matches!(self.letter, 'c' | 's')
    }
    fn is_quadratic_curve_command(&self) -> bool {
        matches!(self.letter, 'q' | 't')
    }
    fn is_curve_command(&self) -> bool {
        matches!(self.letter, 'c' | 's' | 'q' | 't')
    }
    fn is_arc_command(&self) -> bool {
        self.letter == 'a'
    }
    fn is_close_command(&self) -> bool {
        self.letter == 'z'
    }

    fn chunk(&self, index: usize, current_position: Point) -> Command {
        Command {
            letter: self.letter,
            is_relative: self.is_relative,
            parameters: self.parameters[index..index + self.params_count].to_vec(),
            params_count: self.params_count,
            start: current_position,
        }
    }

    fn as_line(&self, new_start: Point) -> Command {
        let converted_parameters =
            self.parameters[self.params_count.min(self.parameters.len())..].to_vec();
        Command {
            letter: 'l',
            is_relative: self.is_relative,
            parameters: converted_parameters,
            params_count: 2,
            start: new_start,
        }
    }
}

fn command_to_params_count(letter: char) -> usize {
    match letter {
        'm' => 2,
        'l' => 2,
        'h' => 1,
        'v' => 1,
        'c' => 6,
        's' => 4,
        'q' => 4,
        't' => 2,
        'a' => 7,
        _ => 0,
    }
}

/// Port of the private `ArcConverter` in SvgPathParser.kt — converts an SVG `a`/`A`
/// arc command to cubic Bézier segments. All of the arc-center math runs in `f64`,
/// exactly like the Kotlin code.
#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    a: f32,
    b: f32,
    theta: f32,
    is_more_than_half: bool,
    is_positive_arc: bool,
) -> Result<Vec<Cubic>, ShapeError> {
    /* Convert rotation angle from degrees to radians */
    let theta_d: f64 = theta as f64 / 180. * core::f64::consts::PI;
    /* Pre-compute rotation matrix entries */
    let cos_theta = theta_d.cos();
    let sin_theta = theta_d.sin();
    /* Transform (x0, y0) and (x1, y1) into unit space */
    /* using (inverse) rotation, followed by (inverse) scale */
    let x0p = (x0 as f64 * cos_theta + y0 as f64 * sin_theta) / a as f64;
    let y0p = (-(x0 as f64) * sin_theta + y0 as f64 * cos_theta) / b as f64;
    let x1p = (x1 as f64 * cos_theta + y1 as f64 * sin_theta) / a as f64;
    let y1p = (-(x1 as f64) * sin_theta + y1 as f64 * cos_theta) / b as f64;

    /* Compute differences and averages */
    let dx = x0p - x1p;
    let dy = y0p - y1p;
    let xm = (x0p + x1p) / 2.;
    let ym = (y0p + y1p) / 2.;
    /* Solve for intersecting unit circles */
    let dsq = dx * dx + dy * dy;
    if dsq == 0.0 {
        return Ok(Vec::new()); /* Points are coincident */
    }
    let disc = 1.0 / dsq - 1.0 / 4.0;
    if disc < 0.0 {
        let adjust = (dsq.sqrt() / 1.99999) as f32;
        /* Points are too far apart */
        return arc_to_cubics(
            x0,
            y0,
            x1,
            y1,
            a * adjust,
            b * adjust,
            theta,
            is_more_than_half,
            is_positive_arc,
        );
    }
    let s = disc.sqrt();
    let sdx = s * dx;
    let sdy = s * dy;
    let (mut cx, mut cy);
    if is_more_than_half == is_positive_arc {
        cx = xm - sdy;
        cy = ym + sdx;
    } else {
        cx = xm + sdy;
        cy = ym - sdx;
    }

    let eta0 = (y0p - cy).atan2(x0p - cx);

    let eta1 = (y1p - cy).atan2(x1p - cx);

    let mut sweep = eta1 - eta0;
    if is_positive_arc != (sweep >= 0.) {
        if sweep > 0. {
            sweep -= 2. * core::f64::consts::PI;
        } else {
            sweep += 2. * core::f64::consts::PI;
        }
    }

    cx *= a as f64;
    cy *= b as f64;
    let tcx = cx;
    cx = cx * cos_theta - cy * sin_theta;
    cy = tcx * sin_theta + cy * cos_theta;

    Ok(arc_to_bezier(cx as f32, cy as f32, a, b, x0, y0, theta_d as f32, eta0 as f32, sweep as f32))
}

/// Converts an arc to cubic Bezier segments. Taken from equations at:
/// http://spaceroots.org/documents/ellipse/node8.html and
/// http://spaceroots.org/documents/ellipse/node22.html.
/// Maximum of 45 degrees per cubic Bezier segment.
#[allow(clippy::too_many_arguments)]
fn arc_to_bezier(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    e1x: f32,
    e1y: f32,
    theta: f32,
    start: f32,
    sweep: f32,
) -> Vec<Cubic> {
    let mut cubics = Vec::new();

    let mut ce1x = e1x;
    let mut ce1y = e1y;
    // kotlin.math.ceil(abs(sweep * 4 / PI)).toInt() — sweep*4 is f32, /PI widens to f64
    let num_segments = ((sweep as f64 * 4. / core::f64::consts::PI).abs().ceil()) as i32;

    let mut eta1 = start;
    let cos_theta = k_cos(theta);
    let sin_theta = k_sin(theta);
    let cos_eta1 = k_cos(eta1);
    let sin_eta1 = k_sin(eta1);
    let mut ep1x = (-rx * cos_theta * sin_eta1) - (ry * sin_theta * cos_eta1);
    let mut ep1y = (-rx * sin_theta * sin_eta1) + (ry * cos_theta * cos_eta1);

    let angle_per_segment = sweep / num_segments as f32;
    for _ in 0..num_segments {
        let eta2 = eta1 + angle_per_segment;
        let sin_eta2 = k_sin(eta2);
        let cos_eta2 = k_cos(eta2);
        let e2x = cx + (rx * cos_theta * cos_eta2) - (ry * sin_theta * sin_eta2);
        let e2y = cy + (rx * sin_theta * cos_eta2) + (ry * cos_theta * sin_eta2);
        let ep2x = -rx * cos_theta * sin_eta2 - ry * sin_theta * cos_eta2;
        let ep2y = -rx * sin_theta * sin_eta2 + ry * cos_theta * cos_eta2;
        let tan_diff2 = k_tan((eta2 - eta1) / 2.);
        let alpha = k_sin(eta2 - eta1) * (k_sqrt(4. + 3. * tan_diff2 * tan_diff2) - 1.) / 3.;
        let q1x = ce1x + alpha * ep1x;
        let q1y = ce1y + alpha * ep1y;
        let q2x = e2x - alpha * ep2x;
        let q2y = e2y - alpha * ep2y;

        cubics.push(Cubic::from_floats(ce1x, ce1y, q1x, q1y, q2x, q2y, e2x, e2y));
        eta1 = eta2;
        ce1x = e2x;
        ce1y = e2y;
        ep1x = ep2x;
        ep1y = ep2y;
    }
    cubics
}

use super::utils::{k_cos, k_sin, k_sqrt, k_tan};

/// The `FeatureSerializer` is used to both serialize and parse [Feature] objects.
/// This is beneficial when you want to re-use [RoundedPolygon] objects created by
/// [SvgPathParser], as parsing serialized [Feature] objects is more performant than
/// using the svg path import.
///
/// Port of `FeatureSerializer` (version 1 format).
pub struct FeatureSerializer;

const SEPARATOR: char = ',';
const CONVEX_CORNER_CHAR: char = 'x';
const CONCAVE_CORNER_CHAR: char = 'o';
const EDGE_CHAR: char = 'n';

impl FeatureSerializer {
    /// Serializes a list of [Feature] objects into a string representation,
    /// adhering to version 1 of the feature serialization format.
    pub fn serialize(features: &[Feature]) -> String {
        let mut result = String::from("V1");
        for feature in features {
            result.push_str(&serialize_feature(feature));
        }
        result
    }

    /// Parses a serialized string representation of [Feature] objects, adhering to
    /// version 1 of the feature serialization format.
    ///
    /// Returns an error if the serialized string lacks sufficient points to create
    /// [Cubic] objects, or if no feature tags can be found.
    pub fn parse(serialized_features: &str) -> Result<Vec<Feature>, ShapeError> {
        // Regex ^\s*V(\d+): find a version prefix.
        let mut tags_search_start = 0;
        let trimmed_start = serialized_features.trim_start();
        if trimmed_start.starts_with('V') {
            let digits: String =
                trimmed_start[1..].chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                tags_search_start =
                    serialized_features.len() - trimmed_start.len() + 1 + digits.len();
            }
        }

        // Find the first [a-zA-Z] tag at or after tagsSearchStart.
        let byte_start = |s: &str, idx: usize| -> usize {
            s.char_indices().nth(idx).map(|(i, _)| i).unwrap_or(s.len())
        };
        let _ = byte_start;

        let search_from = serialized_features
            .char_indices()
            .nth(tags_search_start)
            .map(|(i, _)| i)
            .unwrap_or(serialized_features.len());
        let first_tag_rel =
            serialized_features[search_from..].find(|c: char| c.is_ascii_alphabetic());
        if first_tag_rel.is_none() {
            return Err(ShapeError::new("Could not find any feature tags."));
        }

        // Collect all feature spans: each span starts at a tag char and ends at the
        // next tag char (or end of string).
        let tag_indices: Vec<usize> = serialized_features
            .char_indices()
            .skip_while(|(i, _)| *i < search_from + first_tag_rel.unwrap_or(0))
            .filter(|(_, c)| c.is_ascii_alphabetic())
            .map(|(i, _)| i)
            .collect();
        let mut result = Vec::new();
        for (k, &start) in tag_indices.iter().enumerate() {
            let end = if k + 1 < tag_indices.len() {
                tag_indices[k + 1]
            } else {
                serialized_features.len()
            };
            result.push(parse_feature(serialized_features, start, end)?);
        }
        Ok(result)
    }
}

fn serialize_feature(feature: &Feature) -> String {
    let mut s = String::new();
    match feature {
        Feature::Edge(cubics) => {
            s.push(EDGE_CHAR);
            s.push_str(&serialize_cubics(cubics));
        }
        Feature::Corner { cubics, convex } => {
            s.push(if *convex { CONVEX_CORNER_CHAR } else { CONCAVE_CORNER_CHAR });
            s.push_str(&serialize_cubics(cubics));
        }
    }
    s
}

fn serialize_cubics(cubics: &[Cubic]) -> String {
    // since cubics in a polygon are continuous, we don't need to include the end
    // coordinates as they are the same as the start coordinates of their successors.
    // this is similar to svg path commands.
    let mut result = String::new();
    for cubic in cubics {
        for (i, p) in cubic.points.iter().enumerate().take(6) {
            if i > 0 {
                result.push(SEPARATOR);
            }
            result.push_str(&float_to_kotlin_string(*p));
        }
        // joinToString inserts the separator between windowed chunks: each cubic's
        // first 6 points, so subsequent cubics start after one separator.
        result.push(SEPARATOR);
    }
    let last = cubics.last().unwrap();
    result.push_str(&float_to_kotlin_string(last.anchor1_x()));
    result.push(SEPARATOR);
    result.push_str(&float_to_kotlin_string(last.anchor1_y()));
    result
}

/// Approximates `Float.toString().removeTrailingZeroes()` — the shortest
/// round-trip decimal representation.
fn float_to_kotlin_string(x: f32) -> String {
    let s = x.to_string();
    // Kotlin "1.0" → after removeTrailingZeroes → "1"; Rust already prints "1".
    s
}

fn parse_feature(
    serialized: &str,
    start_index: usize,
    end_index: usize,
) -> Result<Feature, ShapeError> {
    let tag = serialized[start_index..].chars().next().unwrap_or(EDGE_CHAR);
    let cubics = parse_cubics_str(serialized, start_index + tag.len_utf8(), end_index)?;
    match tag {
        EDGE_CHAR => Ok(Feature::Edge(cubics)),
        CONVEX_CORNER_CHAR => Ok(Feature::Corner { cubics, convex: true }),
        CONCAVE_CORNER_CHAR => Ok(Feature::Corner { cubics, convex: false }),
        _ => {
            // Unknown tags default to an edge interpretation in V1.
            Ok(Feature::Edge(cubics))
        }
    }
}

fn parse_cubics_str(
    serialized: &str,
    start_index: usize,
    end_index: usize,
) -> Result<Vec<Cubic>, ShapeError> {
    // Low-level implementation (the Kotlin version avoids allocations the same way):
    // windowed(8, step = 6) over comma-separated floats.
    let window_size = 8;
    let window_step = 6;

    let bytes = serialized.as_bytes();
    let mut point_start = start_index;
    let mut point_end = start_index;
    let mut point_count = 0usize;
    let mut points = [0f32; 8];

    let mut result = Vec::new();
    while point_end < end_index && point_end < bytes.len() {
        if bytes[point_end] != SEPARATOR as u8 {
            point_end += 1;
            continue;
        }

        let tok = &serialized[point_start..point_end];
        points[point_count] = tok.parse::<f32>().map_err(|_| ShapeError::new("Invalid number"))?;
        point_count += 1;
        point_start = point_end + 1;

        if point_count == window_size {
            result.push(Cubic { points });
            let next_start_x = points[window_size - 2];
            let next_start_y = points[window_size - 1];
            points = [0f32; 8];
            points[0] = next_start_x;
            points[1] = next_start_y;
            point_count -= window_step;
        }

        point_end += 1;
    }

    if point_count + 1 != window_size {
        return Err(ShapeError::new("Received a feature with an insufficient amount of numbers"));
    }

    // add last point and last cubic
    let tok = &serialized[point_start..point_end.min(serialized.len())];
    points[window_size - 1] = tok.parse::<f32>().map_err(|_| ShapeError::new("Invalid number"))?;
    result.push(Cubic { points });
    Ok(result)
}

// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore akhn vkrn valt vpai DSIG subsetter VARC varc gvar varstore GDEF HVAR VVAR MVAR COLR repointing reserializes varidx subtables loca glyf subsetted subsetting instancer

//! Partial instancing for embedded variable fonts: pin the fvar axes a build
//! never varies at their default value.
//!
//! Only axes pinned at their fvar default are supported: those are precisely
//! the axes no request in the build can carry, so their normalized coordinate
//! is always 0. The transform is then lossless — a tuple or region whose
//! pinned peak is 0 loses a coordinate it never used; a peak off 0 is dead at
//! coordinate 0; an intermediate straddling 0 rescales its deltas by the tent
//! scalar. Table bytes that don't contain variation data are copied verbatim;
//! the subsetter that runs after this pass drops the slack.

use fontcull_write_fonts::FontBuilder;
use fontcull_write_fonts::read::types::{F2Dot14, GlyphId, Tag};
use fontcull_write_fonts::read::{FontData, FontRead, FontRef, TableProvider};
use std::collections::BTreeSet;

fn be_u16(data: &[u8], pos: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(pos..pos + 2)?.try_into().ok()?))
}

fn be_u32(data: &[u8], pos: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(pos..pos + 4)?.try_into().ok()?))
}

/// Pins every fvar axis of `data` whose tag is in `drop_tags` at the axis'
/// default value and returns the instanced font as a standalone file. `index`
/// selects the face inside a collection. Returns `None` when the font can't be
/// instanced (missing fvar, CFF2 outlines, an avar v2 map, a malformed table) —
/// the caller then embeds the unpinned data.
pub fn pin_axes_to_defaults(data: &[u8], index: u32, drop_tags: &BTreeSet<Tag>) -> Option<Vec<u8>> {
    if drop_tags.is_empty() {
        return None;
    }
    let font = FontRef::from_index(data, index).ok()?;
    let axes = font.fvar().ok()?.axis_instance_arrays().ok()?.axes();
    let pinned: Vec<bool> = axes.iter().map(|axis| drop_tags.contains(&axis.axis_tag())).collect();
    // Pinning every axis leaves a static font that can't serve the dynamic
    // binding that triggered embedding; keep the unpinned data instead.
    if pinned.iter().all(|p| !*p) || pinned.iter().all(|p| *p) {
        return None;
    }
    // CFF2 and VARC keep their variation stores inside structures this pass
    // doesn't rewrite.
    if font.cff2().is_ok() || font.varc().is_ok() {
        return None;
    }

    let mut builder = FontBuilder::new();
    for record in font.table_directory.table_records() {
        let tag = record.tag();
        let bytes: Vec<u8> = match tag {
            t if t == Tag::new(b"fvar") => rewrite_fvar(font.data_for_tag(t)?.as_bytes(), &pinned)?,
            t if t == Tag::new(b"avar") => rewrite_avar(font.data_for_tag(t)?.as_bytes(), &pinned)?,
            t if t == Tag::new(b"gvar") => {
                rewrite_gvar(&font, font.data_for_tag(t)?.as_bytes(), &pinned)?
            }
            t if is_varstore_owner(&t) => {
                rewrite_store_owner(&t, font.data_for_tag(t)?.as_bytes(), &pinned)?
            }
            t => font.data_for_tag(t)?.as_bytes().to_vec(),
        };
        builder.add_raw(tag, bytes);
    }
    Some(builder.build())
}

fn is_varstore_owner(tag: &Tag) -> bool {
    [b"GDEF", b"HVAR", b"VVAR", b"MVAR", b"COLR", b"BASE"].iter().any(|t| tag == &Tag::new(t))
}

/// The scalar a region tent `(start, peak, end)` contributes at normalized
/// coordinate 0 — the value every pinned coordinate takes here.
fn scalar_at_zero(start: f32, peak: f32, end: f32) -> f32 {
    if 0.0 < start || 0.0 > end {
        return 0.0;
    }
    if peak == 0.0 {
        return 1.0;
    }
    if peak > 0.0 { (0.0 - start) / (peak - start) } else { (end - 0.0) / (end - peak) }
}

/// Rewrites fvar: drops the pinned axis records and the pinned coordinates
/// out of every named instance. Rebuilt from the table bytes since the spec
/// layout is fixed.
fn rewrite_fvar(fvar: &[u8], pinned: &[bool]) -> Option<Vec<u8>> {
    let axes_offset = be_u16(fvar, 4)? as usize;
    let axis_count = be_u16(fvar, 8)? as usize;
    let axis_size = be_u16(fvar, 10)? as usize;
    let instance_count = be_u16(fvar, 12)? as usize;
    let instance_size = be_u16(fvar, 14)? as usize;
    if pinned.len() != axis_count || axis_size < 20 || instance_size < 4 + 4 * axis_count {
        return None;
    }
    let kept_count = axis_count - pinned.iter().filter(|p| **p).count();
    let has_postscript_name = instance_size > 4 + 4 * axis_count;
    let new_instance_size = 4 + 4 * kept_count + if has_postscript_name { 2 } else { 0 };

    let mut out = Vec::new();
    out.extend_from_slice(fvar.get(0..4)?); // version
    out.extend_from_slice(&16u16.to_be_bytes()); // axesArrayOffset
    out.extend_from_slice(fvar.get(6..8)?); // reserved (countSizePairs)
    out.extend_from_slice(&(kept_count as u16).to_be_bytes());
    out.extend_from_slice(&(axis_size as u16).to_be_bytes());
    out.extend_from_slice(&(instance_count as u16).to_be_bytes());
    out.extend_from_slice(&(new_instance_size as u16).to_be_bytes());
    for (i, pinned_axis) in pinned.iter().enumerate() {
        if !pinned_axis {
            out.extend_from_slice(
                fvar.get(axes_offset + i * axis_size..axes_offset + (i + 1) * axis_size)?,
            );
        }
    }
    let instances_offset = axes_offset + axis_count * axis_size;
    for i in 0..instance_count {
        let record = fvar.get(
            instances_offset + i * instance_size..instances_offset + (i + 1) * instance_size,
        )?;
        out.extend_from_slice(&record[0..4]); // subfamily name + flags
        for (j, pinned_axis) in pinned.iter().enumerate() {
            if !pinned_axis {
                out.extend_from_slice(&record[4 + 4 * j..4 + 4 * j + 4]);
            }
        }
        if has_postscript_name {
            out.extend_from_slice(&record[4 + 4 * axis_count..4 + 4 * axis_count + 2]);
        }
    }
    Some(out)
}

/// Rewrites avar: drops the pinned axes' segment maps. An avar version 2 table
/// carries a varIdxMap and a delta store keyed to axis order — bail out rather
/// than remap them.
fn rewrite_avar(avar: &[u8], pinned: &[bool]) -> Option<Vec<u8>> {
    let minor = be_u16(avar, 2)?;
    let axis_count = be_u16(avar, 6)? as usize;
    if pinned.len() != axis_count {
        return None;
    }
    let mut pos = 8;
    let mut maps = Vec::with_capacity(axis_count);
    for _ in 0..axis_count {
        let count = be_u16(avar, pos)? as usize;
        pos += 2;
        let map = avar.get(pos..pos + 4 * count)?;
        pos += 4 * count;
        maps.push((count, map));
    }
    if minor >= 2 && (be_u32(avar, pos)? != 0 || be_u32(avar, pos + 4)? != 0) {
        // varIdxMapOffset + itemVariationStoreOffset — both must be unset for
        // a pinning rewrite.
        return None;
    }
    let kept_count = axis_count - pinned.iter().filter(|p| **p).count();
    let mut out = Vec::new();
    out.extend_from_slice(&1u16.to_be_bytes()); // major
    out.extend_from_slice(&0u16.to_be_bytes()); // minor
    out.extend_from_slice(&0u16.to_be_bytes()); // reserved
    out.extend_from_slice(&(kept_count as u16).to_be_bytes());
    for (i, pinned_axis) in pinned.iter().enumerate() {
        if !pinned_axis {
            let (count, map) = maps[i];
            out.extend_from_slice(&(count as u16).to_be_bytes());
            out.extend_from_slice(map);
        }
    }
    Some(out)
}

/// The `(byte offset, width)` of the field pointing at the table's
/// ItemVariationStore, or `None` for a version that can't hold one. Positions
/// are the OpenType layouts for GDEF 1.3, HVAR/VVAR, MVAR, COLR 1.0 and
/// BASE 1.1.
fn varstore_field(tag: &Tag, table: &[u8]) -> Option<(usize, usize)> {
    let version = be_u32(table, 0)?;
    match tag.to_be_bytes().as_slice() {
        b"GDEF" => (version >= 0x00010003).then_some((14, 4)),
        b"HVAR" | b"VVAR" => Some((4, 4)),
        b"MVAR" => Some((10, 2)),
        b"COLR" => (version == 0x00010000).then_some((32, 4)),
        b"BASE" => (version >= 0x00010001).then_some((12, 4)),
        _ => None,
    }
}

/// Rebuilds the ItemVariationStore of a table and splices it in by appending
/// the new store at the table end and repointing the offset field; the dead
/// bytes are dropped when the subsetter reserializes the table.
fn rewrite_store_owner(tag: &Tag, table: &[u8], pinned: &[bool]) -> Option<Vec<u8>> {
    // Table versions that predate a varstore field carry no store — pass
    // them through rather than bailing.
    let Some((field_offset, field_size)) = varstore_field(tag, table) else {
        return Some(table.to_vec());
    };
    let raw_offset = match field_size {
        4 => be_u32(table, field_offset)? as usize,
        _ => be_u16(table, field_offset)? as usize,
    };
    if raw_offset == 0 {
        return Some(table.to_vec());
    }
    let store = rewrite_variation_store(table.get(raw_offset..)?, pinned)?;
    let mut out = table.to_vec();
    let new_offset = out.len();
    if field_size == 2 && new_offset > u16::MAX as usize {
        return None;
    }
    out.extend_from_slice(&store);
    match field_size {
        4 => {
            out[field_offset..field_offset + 4].copy_from_slice(&(new_offset as u32).to_be_bytes())
        }
        _ => {
            out[field_offset..field_offset + 2].copy_from_slice(&(new_offset as u16).to_be_bytes())
        }
    }
    Some(out)
}

/// Rebuilds an ItemVariationStore with the pinned axes dropped: every region
/// keeps only its kept-axis coordinates, and each ItemVariationData drops the
/// columns whose regions are dead at the pin location and rescales columns
/// whose regions are partially active. Region indices are stable, so varidx
/// references elsewhere in the font stay valid.
fn rewrite_variation_store(store: &[u8], pinned: &[bool]) -> Option<Vec<u8>> {
    use fontcull_write_fonts::read::tables::variations::ItemVariationStore;
    let store_table = ItemVariationStore::read(FontData::new(store)).ok()?;
    let region_list = store_table.variation_region_list().ok()?;
    let kept_count = pinned.iter().filter(|p| !**p).count();

    let mut regions_bytes = Vec::new();
    regions_bytes.extend_from_slice(&(kept_count as u16).to_be_bytes());
    let regions: Vec<_> = region_list.variation_regions().iter().collect::<Result<_, _>>().ok()?;
    regions_bytes.extend_from_slice(&(regions.len() as u16).to_be_bytes());
    let mut region_factors = Vec::with_capacity(regions.len());
    for region in &regions {
        let region_axes = region.region_axes();
        if region_axes.len() != pinned.len() {
            return None;
        }
        let mut factor = 1.0f32;
        for (i, axis) in region_axes.iter().enumerate() {
            if pinned[i] {
                factor *= scalar_at_zero(
                    axis.start_coord().to_f32(),
                    axis.peak_coord().to_f32(),
                    axis.end_coord().to_f32(),
                );
            } else {
                for coord in [axis.start_coord(), axis.peak_coord(), axis.end_coord()] {
                    regions_bytes.extend_from_slice(&coord.to_bits().to_be_bytes());
                }
            }
        }
        region_factors.push(factor);
    }

    let mut var_datas: Vec<Vec<u8>> = Vec::new();
    for i in 0..store_table.item_variation_data_count() {
        let Some(Ok(var_data)) = store_table.item_variation_data().get(i as usize) else {
            return None;
        };
        let item_count = var_data.item_count() as usize;
        // Live columns: (position in region_indexes, region index, scale)
        // for regions still alive at the pin. The position is the column
        // index inside the packed delta set.
        let mut columns: Vec<(usize, u16, f32)> = Vec::new();
        for (col, index) in var_data.region_indexes().iter().enumerate() {
            let factor = *region_factors.get(index.get() as usize)?;
            if factor != 0.0 {
                columns.push((col, index.get(), factor));
            }
        }
        if columns.is_empty() {
            // VarData can't be dropped (varidx space is positional); emit it
            // with no regions so every item yields no deltas.
            let mut out = Vec::with_capacity(6);
            out.extend_from_slice(&(item_count as u16).to_be_bytes());
            out.extend_from_slice(&0u16.to_be_bytes());
            out.extend_from_slice(&0u16.to_be_bytes());
            var_datas.push(out);
            continue;
        }
        let mut rows: Vec<Vec<i32>> = Vec::with_capacity(item_count);
        for item in 0..item_count as u16 {
            let mut row = Vec::with_capacity(columns.len());
            for &(col, _, factor) in &columns {
                let delta = var_data.delta_set(item).nth(col)?;
                row.push((delta as f32 * factor).round() as i32);
            }
            rows.push(row);
        }
        // Wide columns must lead in the delta set: the format stores
        // wordDeltaCount wide entries then narrow ones.
        let column_max = |c: usize| rows.iter().map(|r| r[c].abs()).max().unwrap_or(0);
        let needs_i32 = (0..columns.len()).any(|c| column_max(c) > i16::MAX as i32);
        let (wide_max, wide_fmt, narrow_fmt) =
            if needs_i32 { (i16::MAX as i32, 4usize, 2usize) } else { (i8::MAX as i32, 2, 1) };
        let mut order: Vec<usize> = (0..columns.len()).collect();
        order.sort_by_key(|c| core::cmp::Reverse(column_max(*c) > wide_max));
        let word_delta_count =
            order.iter().take_while(|c| column_max(**c) > wide_max).count() as u16;

        let mut out = Vec::new();
        out.extend_from_slice(&(item_count as u16).to_be_bytes());
        out.extend_from_slice(
            &(word_delta_count | if needs_i32 { 0x8000 } else { 0 }).to_be_bytes(),
        );
        out.extend_from_slice(&(columns.len() as u16).to_be_bytes());
        for c in &order {
            out.extend_from_slice(&columns[*c].1.to_be_bytes());
        }
        for row in &rows {
            for (pos, c) in order.iter().enumerate() {
                let delta = row[*c];
                match if pos < word_delta_count as usize { wide_fmt } else { narrow_fmt } {
                    4 => out.extend_from_slice(&delta.to_be_bytes()),
                    2 => out.extend_from_slice(&(delta as i16).to_be_bytes()),
                    _ => out.push(delta as i8 as u8),
                }
            }
        }
        var_datas.push(out);
    }

    // Header, then the region list, then the VarData subtables.
    let region_list_offset = 2 + 4 + 2 + 4 * var_datas.len();
    let mut body = Vec::new();
    let mut out = Vec::new();
    out.extend_from_slice(&1u16.to_be_bytes()); // format
    out.extend_from_slice(&(region_list_offset as u32).to_be_bytes());
    out.extend_from_slice(&(var_datas.len() as u16).to_be_bytes());
    for data in &var_datas {
        out.extend_from_slice(
            &((region_list_offset + regions_bytes.len() + body.len()) as u32).to_be_bytes(),
        );
        body.extend_from_slice(data);
    }
    out.extend_from_slice(&regions_bytes);
    out.extend_from_slice(&body);
    Some(out)
}

// --- gvar ---------------------------------------------------------------

const EMBEDDED_PEAK_TUPLE: u16 = 0x8000;
const INTERMEDIATE_REGION: u16 = 0x4000;
const PRIVATE_POINT_NUMBERS: u16 = 0x2000;
const SHARED_POINT_NUMBERS: u16 = 0x8000; // glyph-level flag
const LONG_OFFSETS: u16 = 0x0001;

/// Rewrites gvar with the pinned axes dropped. Tuple deltas are copied byte
/// for byte — only the coordinate records shrink — except for tuples whose
/// pinned axes have intermediates straddling the default, which are rescaled
/// and repacked. Every kept tuple is re-emitted with an embedded peak, so the
/// shared tuple array is left empty.
fn rewrite_gvar(font: &FontRef, gvar: &[u8], pinned: &[bool]) -> Option<Vec<u8>> {
    let axis_count = be_u16(gvar, 4)? as usize;
    let glyph_count = be_u16(gvar, 12)? as usize;
    let long_offsets = be_u16(gvar, 14)? & LONG_OFFSETS != 0;
    let data_offset = be_u32(gvar, 16)? as usize;
    if pinned.len() != axis_count {
        return None;
    }
    let kept_count = axis_count - pinned.iter().filter(|p| **p).count();

    let glyph_offset = |i: usize| -> Option<usize> {
        let pos = 20 + if long_offsets { 4 * i } else { 2 * i };
        let raw = if long_offsets {
            be_u32(gvar, pos)? as usize
        } else {
            be_u16(gvar, pos)? as usize * 2
        };
        Some(data_offset + raw)
    };

    let mut glyph_datas: Vec<Vec<u8>> = Vec::with_capacity(glyph_count);
    for gid in 0..glyph_count {
        let (start, end) = (glyph_offset(gid)?, glyph_offset(gid + 1)?);
        let glyph = gvar.get(start..end)?;
        if glyph.is_empty() {
            glyph_datas.push(Vec::new());
            continue;
        }
        glyph_datas.push(rewrite_glyph_variation_data(font, glyph, gid, pinned, gvar)?);
    }

    // Assemble: header, u32 offsets, no shared tuples, glyph data.
    let new_data_offset = 20 + 4 * (glyph_count + 1);
    let mut out = Vec::new();
    out.extend_from_slice(&0x00010000u32.to_be_bytes()); // version
    out.extend_from_slice(&(kept_count as u16).to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // sharedTupleCount
    // offsetToSharedTuples must point somewhere valid — a null offset fails
    // offset resolution even with sharedTupleCount == 0.
    out.extend_from_slice(&(new_data_offset as u32).to_be_bytes());
    out.extend_from_slice(&(glyph_count as u16).to_be_bytes());
    out.extend_from_slice(&LONG_OFFSETS.to_be_bytes());
    out.extend_from_slice(&(new_data_offset as u32).to_be_bytes());
    let mut at = 0u32;
    for data in &glyph_datas {
        out.extend_from_slice(&at.to_be_bytes());
        at += data.len() as u32;
    }
    out.extend_from_slice(&at.to_be_bytes());
    for data in &glyph_datas {
        out.extend_from_slice(data);
    }
    Some(out)
}

/// Rewrites one glyph's GlyphVariationData.
fn rewrite_glyph_variation_data(
    font: &FontRef,
    glyph: &[u8],
    gid: usize,
    pinned: &[bool],
    gvar: &[u8],
) -> Option<Vec<u8>> {
    let axis_count = pinned.len();
    let shared_offset = be_u32(gvar, 8)? as usize;
    let count_and_flag = be_u16(glyph, 0)?;
    let shared_points = count_and_flag & SHARED_POINT_NUMBERS != 0;
    let tuple_count = (count_and_flag & 0x0fff) as usize;
    let data_offset = be_u16(glyph, 2)? as usize;

    struct TupleHeader<'a> {
        size: usize,
        index: u16,
        /// The resolved peak coordinates — embedded or from the shared array.
        peak: &'a [u8],
        intermediate: Option<Vec<u8>>,
    }
    let mut headers: Vec<TupleHeader> = Vec::with_capacity(tuple_count);
    let mut pos = 4;
    for _ in 0..tuple_count {
        let size = be_u16(glyph, pos)? as usize;
        let index = be_u16(glyph, pos + 2)?;
        pos += 4;
        let peak = if index & EMBEDDED_PEAK_TUPLE != 0 {
            let bytes = glyph.get(pos..pos + 2 * axis_count)?;
            pos += 2 * axis_count;
            bytes
        } else {
            let idx = (index & 0x0fff) as usize;
            gvar.get(
                shared_offset + idx * 2 * axis_count..shared_offset + (idx + 1) * 2 * axis_count,
            )?
        };
        let intermediate = if index & INTERMEDIATE_REGION != 0 {
            let bytes = glyph.get(pos..pos + 4 * axis_count)?;
            pos += 4 * axis_count;
            Some(bytes.to_vec())
        } else {
            None
        };
        headers.push(TupleHeader { size, index, peak, intermediate });
    }
    if pos > data_offset || data_offset > glyph.len() {
        return None;
    }

    let serialized = glyph.get(data_offset..)?;
    // A leading packed point list shared by tuples without private numbers.
    let (shared_list, shared_count) = if shared_points {
        let (count, len) = unpack_point_numbers(serialized)?;
        (&serialized[..len], Some(count))
    } else {
        (&serialized[..0], None)
    };
    let mut body = &serialized[shared_list.len()..];

    struct Kept {
        index: u16,
        peak: Vec<u8>,
        intermediate: Option<Vec<u8>>,
        data: Vec<u8>,
    }
    let kept_axis_count = axis_count - pinned.iter().filter(|p| **p).count();
    let mut kept: Vec<Kept> = Vec::new();
    for header in &headers {
        // Serialized tuple data follows the headers in the same order —
        // advance `body` for every tuple, dropped or kept.
        let tuple_data = body.get(..header.size)?;
        body = &body[header.size..];
        let mut factor = 1.0f32;
        let mut drop = false;
        let mut new_peak = Vec::with_capacity(2 * kept_axis_count);
        for (i, pinned_axis) in pinned.iter().enumerate() {
            let p = F2Dot14::from_bits(be_u16(header.peak, 2 * i)? as i16).to_f32();
            if !pinned_axis {
                new_peak.extend_from_slice(&header.peak[2 * i..2 * i + 2]);
                continue;
            }
            let (start, end) = match &header.intermediate {
                Some(bytes) => (
                    F2Dot14::from_bits(be_u16(bytes, 2 * i)? as i16).to_f32(),
                    F2Dot14::from_bits(be_u16(bytes, 2 * i + 2 * axis_count)? as i16).to_f32(),
                ),
                None => (p.min(0.0), p.max(0.0)),
            };
            let f = scalar_at_zero(start, p, end);
            if f == 0.0 {
                drop = true;
                break;
            }
            factor *= f;
        }
        if drop {
            continue;
        }
        // Intermediate regions store all starts then all ends.
        let new_intermediate = header.intermediate.as_ref().map(|bytes| {
            let mut out = Vec::with_capacity(4 * kept_axis_count);
            for half in [0usize, 2 * axis_count] {
                for (i, pinned_axis) in pinned.iter().enumerate() {
                    if !pinned_axis {
                        out.extend_from_slice(&bytes[half + 2 * i..half + 2 * i + 2]);
                    }
                }
            }
            out
        });
        let data = if factor == 1.0 {
            tuple_data.to_vec()
        } else {
            rescale_tuple_data(font, gid, tuple_data, header.index, shared_count, factor)?
        };
        kept.push(Kept {
            index: EMBEDDED_PEAK_TUPLE
                | (header.index & PRIVATE_POINT_NUMBERS)
                | if new_intermediate.is_some() { INTERMEDIATE_REGION } else { 0 },
            peak: new_peak,
            intermediate: new_intermediate,
            data,
        });
    }

    let mut out = Vec::new();
    out.extend_from_slice(
        &((kept.len() as u16) | if shared_points { SHARED_POINT_NUMBERS } else { 0 }).to_be_bytes(),
    );
    let headers_len: usize = kept
        .iter()
        .map(|k| 4 + k.peak.len() + k.intermediate.as_ref().map_or(0, |i| i.len()))
        .sum();
    out.extend_from_slice(&((4 + headers_len) as u16).to_be_bytes());
    for k in &kept {
        out.extend_from_slice(&(k.data.len() as u16).to_be_bytes());
        out.extend_from_slice(&k.index.to_be_bytes());
        out.extend_from_slice(&k.peak);
        if let Some(intermediate) = &k.intermediate {
            out.extend_from_slice(intermediate);
        }
    }
    out.extend_from_slice(shared_list);
    for k in &kept {
        out.extend_from_slice(&k.data);
    }
    Some(out)
}

/// Reads a PackedPointNumbers list, returning `(point count, bytes consumed)`.
/// A count of 0 means "all points" — the caller resolves it against the
/// glyph's point space.
fn unpack_point_numbers(data: &[u8]) -> Option<(u32, usize)> {
    let mut pos = 0usize;
    let mut count = *data.get(pos)? as u32;
    pos += 1;
    if count & 0x80 != 0 {
        // Wide count: the low 7 bits are the high byte of a 15-bit count.
        count = ((count & 0x7f) << 8) | (*data.get(pos)? as u32);
        pos += 1;
    }
    let mut points_seen = 0u32;
    while points_seen < count {
        let run = *data.get(pos)?;
        pos += 1;
        let wide = run & 0x80 != 0;
        let run_len = (run & 0x7f) as u32 + 1;
        pos += run_len as usize * if wide { 2 } else { 1 };
        points_seen += run_len;
    }
    Some((count, pos))
}

/// Unpacks a PackedDeltas stream covering `count` points. Returns the values
/// and the number of bytes consumed.
fn unpack_deltas(data: &[u8], count: usize) -> Option<(Vec<i32>, usize)> {
    let mut pos = 0;
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let control = *data.get(pos)?;
        pos += 1;
        // Zero runs carry a 7-bit count; byte/word runs carry 6 bits.
        let run_len = (control & if control & 0x80 != 0 { 0x7f } else { 0x3f }) as usize + 1;
        if control & 0x80 != 0 {
            out.resize(out.len() + run_len, 0);
        } else if control & 0x40 != 0 {
            for _ in 0..run_len {
                out.push(be_u16(data, pos).map(|v| v as i16 as i32)?);
                pos += 2;
            }
        } else {
            for _ in 0..run_len {
                out.push(*data.get(pos)? as i8 as i32);
                pos += 1;
            }
        }
    }
    Some((out, pos))
}

/// Repacks scaled deltas into zero/i8/i16 runs of at most 64 entries.
fn pack_deltas(deltas: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < deltas.len() {
        let fits_i8 = |d: i32| d >= i8::MIN as i32 && d <= i8::MAX as i32;
        let kind_of = |d: i32| {
            if d == 0 {
                0
            } else if fits_i8(d) {
                1
            } else {
                2
            }
        };
        let kind = kind_of(deltas[i]);
        let cap = if kind == 0 { 128 } else { 64 };
        let mut j = i;
        while j < i + cap && j < deltas.len() && kind_of(deltas[j]) == kind {
            j += 1;
        }
        let run_len = (j - i) as u8;
        match kind {
            0 => out.push(0x80 | (run_len - 1)),
            1 => {
                out.push(run_len - 1);
                for d in &deltas[i..j] {
                    out.push(*d as i8 as u8);
                }
            }
            _ => {
                out.push(0x40 | (run_len - 1));
                for d in &deltas[i..j] {
                    out.extend_from_slice(&(*d as i16).to_be_bytes());
                }
            }
        }
        i = j;
    }
    out
}

/// The number of outline points a gvar tuple covers — the glyph's own points,
/// or one per component for composites — plus the four phantom points.
fn glyph_point_count(font: &FontRef, gid: u32) -> Option<usize> {
    use fontcull_write_fonts::read::tables::glyf::Glyph;
    let loca = font.loca(None).ok()?;
    let glyf = font.glyf().ok()?;
    let glyph = loca.get_glyf(GlyphId::new(gid), &glyf).ok()??;
    let count = match glyph {
        Glyph::Simple(simple) => simple.num_points(),
        Glyph::Composite(composite) => composite.component_glyphs_and_flags().count(),
    };
    Some(count + 4)
}

/// Rescales a kept tuple's deltas by `factor`, preserving the point list.
/// `shared_point_count` is the glyph's shared list length when the header
/// flag is set; a count of 0 means all points.
fn rescale_tuple_data(
    font: &FontRef,
    gid: usize,
    data: &[u8],
    index: u16,
    shared_point_count: Option<u32>,
    factor: f32,
) -> Option<Vec<u8>> {
    let (points_bytes, point_count) = if index & PRIVATE_POINT_NUMBERS != 0 {
        let (count, len) = unpack_point_numbers(data)?;
        let count = if count == 0 { glyph_point_count(font, gid as u32)? } else { count as usize };
        (&data[..len], count)
    } else if let Some(count) = shared_point_count {
        let count = if count == 0 { glyph_point_count(font, gid as u32)? } else { count as usize };
        (&data[..0], count)
    } else {
        (&data[..0], glyph_point_count(font, gid as u32)?)
    };
    let body = &data[points_bytes.len()..];
    let (x_deltas, used) = unpack_deltas(body, point_count)?;
    let (y_deltas, _) = unpack_deltas(body.get(used..)?, point_count)?;
    let mut out = Vec::new();
    out.extend_from_slice(points_bytes);
    out.extend_from_slice(&pack_deltas(
        &x_deltas.iter().map(|d| (*d as f32 * factor).round() as i32).collect::<Vec<_>>(),
    ));
    out.extend_from_slice(&pack_deltas(
        &y_deltas.iter().map(|d| (*d as f32 * factor).round() as i32).collect::<Vec<_>>(),
    ));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_common::sharedfontique::skrifa;
    use i_slint_common::sharedfontique::skrifa::{
        MetadataProvider, instance::Size, metrics::GlyphMetrics,
    };

    const ROBOTO_FLEX: &[u8] = include_bytes!("../../../../tests/screenshots/fonts/RobotoFlex.ttf");

    fn s_tag(tag: Tag) -> skrifa::Tag {
        skrifa::Tag::from_be_bytes(tag.to_be_bytes())
    }

    fn advance_and_bounds(
        data: &[u8],
        glyph: u32,
        settings: &[(skrifa::Tag, f32)],
    ) -> (f32, [f32; 4]) {
        let font = skrifa::FontRef::new(data).unwrap();
        let location = font.axes().location(settings.iter().copied());
        let metrics = GlyphMetrics::new(&font, Size::unscaled(), &location);
        let gid = skrifa::GlyphId::new(glyph);
        let advance = metrics.advance_width(gid).unwrap_or_default();
        let bbox = metrics.bounds(gid).unwrap_or_default();
        (advance, [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max])
    }

    /// Pinning every non-kept axis of Roboto Flex at its default must produce
    /// a font that renders identically at any (kept-axis) location: gvar
    /// tuples for dropped axes contribute nothing at coordinate 0, HVAR/GDEF
    /// varstore columns for dead regions scale to 0, and avar for the kept
    /// axes survives. The check compares advance widths and outlines' bounding
    /// boxes on the instanced font against the original at the same location.
    #[test]
    fn partial_instancing_preserves_rendering() {
        let original = FontRef::new(ROBOTO_FLEX).unwrap();
        let keep: BTreeSet<Tag> = [b"wght", b"opsz"].iter().map(|t| Tag::new(*t)).collect();
        let drop_tags: BTreeSet<Tag> = original
            .fvar()
            .unwrap()
            .axis_instance_arrays()
            .unwrap()
            .axes()
            .iter()
            .map(|axis| axis.axis_tag())
            .filter(|tag| !keep.contains(tag))
            .collect();
        assert_eq!(drop_tags.len(), 11);

        let instanced = pin_axes_to_defaults(ROBOTO_FLEX, 0, &drop_tags).unwrap();
        assert!(instanced.len() < ROBOTO_FLEX.len());
        let instanced_font = skrifa::FontRef::new(&instanced).unwrap();
        let axes: Vec<_> = instanced_font.axes().iter().map(|a| a.tag()).collect();
        assert_eq!(axes, vec![s_tag(Tag::new(b"opsz")), s_tag(Tag::new(b"wght"))]);

        // 'A' and 'g' both vary on wght/opsz and on some dropped axes.
        let font = skrifa::FontRef::new(ROBOTO_FLEX).unwrap();
        let gid_a = font.charmap().map('A').unwrap().to_u32();
        let gid_g = font.charmap().map('g').unwrap().to_u32();
        let wght = s_tag(Tag::new(b"wght"));
        let opsz = s_tag(Tag::new(b"opsz"));
        for settings in [vec![(wght, 300.0), (opsz, 14.0)], vec![(wght, 700.0), (opsz, 72.0)]] {
            for gid in [gid_a, gid_g] {
                let (adv, bbox) = advance_and_bounds(ROBOTO_FLEX, gid, &settings);
                let (adv_i, bbox_i) = advance_and_bounds(&instanced, gid, &settings);
                assert!(
                    (adv - adv_i).abs() < 0.51,
                    "advance differs at {settings:?}: {adv} vs {adv_i}"
                );
                for (a, b) in bbox.iter().zip(bbox_i.iter()) {
                    assert!(
                        (a - b).abs() < 1.01,
                        "bounds differ at {settings:?} for glyph {gid}: {bbox:?} vs {bbox_i:?}"
                    );
                }
            }
        }

        // The kept axes must still vary: wght 900 differs from wght 400.
        let thin: &[(skrifa::Tag, f32)] = &[(s_tag(Tag::new(b"wght")), 400.0)];
        let thick: &[(skrifa::Tag, f32)] = &[(s_tag(Tag::new(b"wght")), 900.0)];
        let (a1, b1) = advance_and_bounds(&instanced, gid_a, thin);
        let (a2, b2) = advance_and_bounds(&instanced, gid_a, thick);
        assert!((a1 - a2).abs() > 1.0 || b1 != b2);
    }

    /// The full pipeline — pin, then subset — must measurably shrink a real
    /// variable font: Roboto Flex pinned to {{wght, opsz}} and subset to ASCII
    /// printable drops most of its bytes. `GSF_FONT` overrides the input to
    /// measure the Material sample font end to end; a relative path resolves
    /// against the crate directory.
    #[test]
    fn pin_then_subset_shrinks() {
        let (data, keep_tags): (std::borrow::Cow<[u8]>, &[&[u8; 4]]) =
            match std::env::var("GSF_FONT").ok().map(|p| {
                std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(p)).unwrap()
            }) {
                Some(bytes) => (bytes.into(), &[b"wght", b"opsz"]),
                None => (ROBOTO_FLEX.into(), &[b"wght", b"opsz"]),
            };
        let font = FontRef::new(&data).unwrap();
        let keep: BTreeSet<Tag> = keep_tags.iter().map(|t| Tag::new(*t)).collect();
        let drop_tags: BTreeSet<Tag> = font
            .fvar()
            .unwrap()
            .axis_instance_arrays()
            .unwrap()
            .axes()
            .iter()
            .map(|axis| axis.axis_tag())
            .filter(|tag| !keep.contains(tag))
            .collect();
        let instanced = pin_axes_to_defaults(&data, 0, &drop_tags).unwrap();
        let charset: std::collections::HashSet<char> = (0x20u8..=0x7e).map(char::from).collect();
        let subset = super::super::subset_vector_font(&instanced, 0, &charset).unwrap();
        eprintln!(
            "subset pipeline: {} -> instanced {} -> subsetted {}",
            data.len(),
            instanced.len(),
            subset.len()
        );
        assert!(subset.len() * 4 < data.len());
        assert!(FontRef::new(&subset).is_ok());
    }
}

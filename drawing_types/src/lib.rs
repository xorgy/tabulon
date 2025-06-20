// Copyright 2025 the Tabulon Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Common types for technical drawings.
//!
//! Meant to encompass significant subset of DXF and DWG compatible
//! primitives, as well as new ones.

use peniko::kurbo;

extern crate alloc;
use alloc::{
    collections::btree_map::{BTreeMap, Entry},
    vec::Vec,
};
use core::cmp::Ordering;

/// 3-dimensional vector or point.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

impl PartialEq for Vec3 {
    fn eq(&self, other: &Self) -> bool {
        self.x.total_cmp(&other.x) == Ordering::Equal
            && self.y.total_cmp(&other.y) == Ordering::Equal
            && self.z.total_cmp(&other.z) == Ordering::Equal
    }
}

impl Eq for Vec3 {}

impl Ord for Vec3 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.x
            .total_cmp(&other.x)
            .then(self.y.total_cmp(&other.y))
            .then(self.z.total_cmp(&other.z))
    }
}

impl PartialOrd for Vec3 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<kurbo::Point> for Vec3 {
    #[inline(always)]
    fn from(kurbo::Point { x, y }: kurbo::Point) -> Self {
        Self { x, y, z: 0. }
    }
}

impl From<kurbo::Size> for Vec3 {
    #[inline(always)]
    fn from(
        kurbo::Size {
            width: x,
            height: y,
        }: kurbo::Size,
    ) -> Self {
        Self { x, y, z: 0. }
    }
}

impl From<kurbo::Vec2> for Vec3 {
    #[inline(always)]
    fn from(kurbo::Vec2 { x, y }: kurbo::Vec2) -> Self {
        Self { x, y, z: 0. }
    }
}

impl From<Vec3> for kurbo::Point {
    #[inline(always)]
    fn from(val: Vec3) -> Self {
        kurbo::Point { x: val.x, y: val.y }
    }
}

impl From<Vec3> for kurbo::Size {
    #[inline(always)]
    fn from(val: Vec3) -> Self {
        kurbo::Size {
            width: val.x,
            height: val.y,
        }
    }
}

impl From<Vec3> for kurbo::Vec2 {
    #[inline(always)]
    fn from(val: Vec3) -> Self {
        kurbo::Vec2 { x: val.x, y: val.y }
    }
}

/// 2-dimensional vector or point.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vec2 {
    x: f64,
    y: f64,
}

impl PartialEq for Vec2 {
    fn eq(&self, other: &Self) -> bool {
        self.x.total_cmp(&other.x) == Ordering::Equal
            && self.y.total_cmp(&other.y) == Ordering::Equal
    }
}

impl Eq for Vec2 {}

impl Ord for Vec2 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.x.total_cmp(&other.x).then(self.y.total_cmp(&other.y))
    }
}

impl PartialOrd for Vec2 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<kurbo::Point> for Vec2 {
    #[inline(always)]
    fn from(kurbo::Point { x, y }: kurbo::Point) -> Self {
        Self { x, y }
    }
}

impl From<kurbo::Size> for Vec2 {
    #[inline(always)]
    fn from(
        kurbo::Size {
            width: x,
            height: y,
        }: kurbo::Size,
    ) -> Self {
        Self { x, y }
    }
}

impl From<kurbo::Vec2> for Vec2 {
    #[inline(always)]
    fn from(kurbo::Vec2 { x, y }: kurbo::Vec2) -> Self {
        Self { x, y }
    }
}

impl From<Vec2> for kurbo::Point {
    #[inline(always)]
    fn from(v: Vec2) -> kurbo::Point {
        kurbo::Point { x: v.x, y: v.y }
    }
}

impl From<Vec2> for kurbo::Size {
    #[inline(always)]
    fn from(val: Vec2) -> Self {
        kurbo::Size {
            width: val.x,
            height: val.y,
        }
    }
}

impl From<Vec2> for kurbo::Vec2 {
    #[inline(always)]
    fn from(val: Vec2) -> Self {
        kurbo::Vec2 { x: val.x, y: val.y }
    }
}

/// A triangle.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Triangle([u32; 3]);
/// A quadrilateral.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Quadrilateral([u32; 4]);

/// A handle for an [`Entity`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityHandle(u32);

/// A handle for a [`Brush`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BrushHandle(u32);

/// A brush for a fill or line.
#[repr(C, u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Brush {
    /// Standard RGB with separate alpha.
    Rgba([u8; 4]),
    /// Index of color in the palette, usually `0..=255` is mapped to the ACI palette
    /// or a compatible palette.
    Index(u32),
}

/// A handle for a line width.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineWeightHandle(u32);

/// Entity.
#[repr(C, u8)]
#[derive(Debug, Clone, PartialEq)]
pub enum Entity {
    /// A solid triangle.
    ///
    /// For `SOLID` with three distinct points.
    SolidTriangle {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// The concrete shape.
        triangle: Triangle,
    },
    /// A solid quadrilateral.
    ///
    /// For `SOLID` with four distinct points.
    SolidQuadrilateral {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// The concrete shape.
        quadrilateral: Quadrilateral,
    },
    /// A line segment.
    ///
    /// For `LINE`.
    LineSegment {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// Handle for the line weight.
        weight: LineWeightHandle,
        /// Index of first point.
        a: u32,
        /// Index of second point.
        b: u32,
    },
    /// A basic polyline with one line weight.
    ///
    /// For some cases of `LWPOLYLINE` and `POLYLINE`.
    PolyLine {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// Handle for the line weight.
        weight: LineWeightHandle,
        /// Index of first point in the index buffer.
        first: u32,
        /// Index of last point in the index buffer.
        last: u32,
    },
    /// A spline with the `bulge` parameter.
    ///
    /// For some cases of `LWPOLYLINE` and `POLYLINE`.
    BulgeSpline {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// Handle for the line weight.
        weight: LineWeightHandle,
        /// Index of first point in the index buffer.
        point_first: u32,
        /// Index of last point in the index buffer.
        point_last: u32,
        /// Index of first bulge parameter in the parameter buffer.
        bulge_first: u32,
    },
    /// A B-Spline.
    ///
    /// For `SPLINE`.
    BasisSpline {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// Handle for the line weight.
        weight: LineWeightHandle,
        /// Index of first point in the index buffer.
        point_first: u32,
        /// Index of last point in the index buffer.
        point_last: u32,
        /// Index of the first knot in the parameter buffer.
        knot_first: u32,
        /// Degree of the spline.
        ///
        /// Typically 1, 2, or 3.
        degree: u8,
    },
    /// A circle.
    Circle {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// Handle for the line weight.
        weight: LineWeightHandle,
        /// Index of the center point.
        center: u32,
        /// Index of the radius in the parameter buffer.
        radius: u32,
    },
    /// An elliptical arc.
    ///
    /// For `ELLIPSE`.
    EllipticalArc {
        /// The [`BrushHandle`] for this solid's paint.
        paint: BrushHandle,
        /// Handle for the line weight.
        weight: LineWeightHandle,
        /// Index of the center point.
        center: u32,
        /// Index of the major axis radius in the parameter buffer.
        major_radius: u32,
        /// Index of the minor axis radius in the parameter buffer.
        minor_radius: u32,
        /// Index of the start angle in the parameter buffer.
        start: u32,
        /// Index of the end angle in the parameter buffer.
        end: u32,
        /// Index of the major axis angle in the parameter buffer.
        x_rotation: u32,
    },
    /// A block instance.
    BlockInstance {
        /// [`BrushHandle`] for this instance's block paint.
        paint: BrushHandle,
        /// Handle for this instance's block line weight.
        weight: LineWeightHandle,
        /// Handle of the block to be inserted.
        block: u32,
        /// Index of the insertion point.
        insert_point: u32,
        /// Index of the block x scale in the parameter buffer.
        x_scale: u32,
        /// Index of the block y scale in the parameter buffer.
        y_scale: u32,
        /// Index of the block rotation in the parameter buffer.
        rotation: u32,
    },
}

/// Block definition.
#[repr(C)]
#[derive(Debug, Clone, PartialEq)]
pub struct BlockDefinition {
    /// String id for the human readable name of this block.
    name: u32,
    /// The base point.
    ///
    /// Aligned with the insertion point of an [`Entity::BlockInstance`] prior
    /// to scaling and rotation.
    base_point: Vec2,
    /// Index of the first entity in the entities buffer.
    first_entity: u32,
    /// Index of the last entity in the entities buffer.
    last_entity: u32,
}

/// Builder for deduplicated [`Vec2`] buffers, providing handles.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Vec2BufBuilder {
    map: BTreeMap<Vec2, u32>,
    buf: Vec<Vec2>,
}

impl Vec2BufBuilder {
    /// Get a handle for `v` in the buffer, inserting if it is unique.
    pub fn handle(&mut self, v: Vec2) -> u32 {
        match self.map.entry(v) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let h = self.buf.len() as u32;
                self.buf.push(v);
                entry.insert(h);
                h
            }
        }
    }

    /// Get the value of the point for handle `h`.
    pub fn get(&mut self, h: u32) -> Vec2 {
        self.buf[h as usize]
    }

    /// Consume the builder as `Box<[Vec2]>`.
    pub fn consume(self) -> Box<[Vec2]> {
        self.buf.into_boxed_slice()
    }
}

/// Make a new empty [`Vec2BufBuilder`].
#[unsafe(no_mangle)]
pub extern "C" fn vec2_buf_builder_new() -> *mut Vec2BufBuilder {
    Box::into_raw(Box::new(Vec2BufBuilder::default()))
}

/// Free a [`Vec2BufBuilder`].
#[unsafe(no_mangle)]
pub extern "C" fn vec2_buf_builder_free(ptr: *mut Vec2BufBuilder) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr); // Freed.
        }
    }
}

/// Vec2 buffer.
#[repr(C)]
pub struct Vec2Buf {
    pub data: *mut Vec2,
    pub len: u32,
}

/// Debug print a [`Vec2`].
#[unsafe(no_mangle)]
pub extern "C" fn vec2_debug_print(v: Vec2) {
    println!("{v:?}");
}

/// Consume the buffer of [`Vec2BufBuilder`].
#[unsafe(no_mangle)]
pub extern "C" fn vec2_buf_builder_consume(ptr: *mut Vec2BufBuilder) -> Vec2Buf {
    let builder = unsafe { Box::from_raw(ptr) };
    let mut bs = builder.consume();
    let buf = Vec2Buf {
        data: bs.as_mut_ptr(),
        len: bs.len() as u32,
    };
    core::mem::forget(bs);
    buf
}

/// Get a handle for `v` in the buffer, inserting if it is unique.
///
/// # Safety
/// The `builder` pointer must be valid and non-null.
#[unsafe(no_mangle)]
pub extern "C" fn vec2_buf_builder_handle(builder: *mut Vec2BufBuilder, v: Vec2) -> u32 {
    unsafe { (&mut *builder).handle(v) }
}

/// Get the value of the point for handle `h`.
///
/// # Safety
/// The `builder` pointer must be valid and non-null, and `h` must be valid.
#[unsafe(no_mangle)]
pub extern "C" fn vec2_buf_builder_get(builder: *mut Vec2BufBuilder, h: u32) -> Vec2 {
    unsafe { Vec2BufBuilder::get(&mut *builder, h) }
}

/// Debug print the contents of the [`Vec2BufBuilder`].
///
/// # Safety
/// The `builder` pointer must be valid and non-null.
#[unsafe(no_mangle)]
pub extern "C" fn vec2_buf_builder_debug_print(builder: *mut Vec2BufBuilder) {
    unsafe {
        println!("{:#?}", &*builder);
    }
}

/// FFI Compatible entity buffer.
#[repr(C)]
pub struct EntityBuf {
    pub data: *mut Entity,
    pub len: u32,
}

/// FFI Compatible brush buffer.
#[repr(C)]
pub struct BrushBuf {
    pub data: *mut Brush,
    pub len: u32,
}

/// FFI Compatible line weight buffer.
#[repr(C)]
pub struct LineWeightBuf {
    pub data: *mut u32,
    pub len: u32,
}

/// FFI Compatible parameter buffer.
#[repr(C)]
pub struct ParamBuf {
    pub data: *mut f64,
    pub len: u32,
}

/// FFI Compatibility fat string pointer.
#[repr(C)]
pub struct FatString {
    /// Non terminated UTF-8 string data.
    pub data: *mut u8,
    /// The length of the string data.
    pub len: u32,
}

/// FFI Compatible strings buffer.
///
/// This contains , should be carefully translated to
/// a better form of safe string ids when crossing the FFI boundary.
#[repr(C)]
pub struct StringsBuf {
    pub data: *mut FatString,
    pub len: u32,
}

/// FFI Compatible block definition buffer.
#[repr(C)]
pub struct BlockDefinitionBuf {
    pub data: *mut BlockDefinition,
    pub len: u32,
}

/// Coordinate space.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CoordinateSpace {
    #[default]
    /// Iota ― 1⁄9 nm
    Iota,
    /// Meters.
    Meter,
    /// Inches,
    Inch,
    /// Drawing has absurd units (angstroms, lightyears, etc.) or is unitless.
    AbsurdOrUnitless,
}

/// FFI Compatible Drawing.
///
/// This is provided to make it simpler to produce a drawing with buffers managed
/// from outside the FFI boundary.
#[repr(C)]
pub struct CompatDrawing {
    /// Buffer of extent strings.
    strings: StringsBuf,
    /// Buffer of [`Entity`].
    entities: EntityBuf,
    /// Buffer of [`BlockDefinition`].
    blocks: BlockDefinitionBuf,
    /// Buffer of `f64` parameters.
    params: ParamBuf,
    /// Buffer of points.
    points: Vec2Buf,
    /// Buffer of [`Brush`].
    brushes: BrushBuf,
    /// Buffer of line weights.
    line_weights: LineWeightBuf,
    /// Index of the first entity in the drawing (not block definitions).
    first_drawing_entity: u32,
    /// World coordinate space.
    world_coordinate_space: CoordinateSpace,
}

/// Debug print an [`Entity`].
#[unsafe(no_mangle)]
pub extern "C" fn debug_print_entity(e: &Entity) {
    println!("{e:#?}");
}

/// Drawing.
///
/// This is the Rust equivalent of [`CompatDrawing`].
#[derive(Clone, Debug)]
pub struct Drawing {
    /// Strings indexed by string id.
    pub strings: Vec<Box<str>>,
    /// Entities.
    pub entities: Vec<Entity>,
    /// Block definitions.
    pub blocks: Vec<BlockDefinition>,
    /// Parameters.
    pub params: Vec<f64>,
    /// Points.
    pub points: Vec<Vec2>,
    /// Brushes.
    pub brushes: Vec<Brush>,
    /// Line weights.
    pub line_weights: Vec<u32>,
    /// Index of the first entity in the drawing (not block definitions).
    pub first_drawing_entity: u32,
    /// World coordinate space.
    pub world_coordinate_space: CoordinateSpace,
}

impl From<&CompatDrawing> for Drawing {
    fn from(d: &CompatDrawing) -> Self {
        // Convert strings
        let strings = unsafe {
            core::slice::from_raw_parts(d.strings.data, d.strings.len as usize)
                .iter()
                .map(|fat_str| {
                    let bytes = core::slice::from_raw_parts(fat_str.data, fat_str.len as usize);
                    Box::from(String::from_utf8_lossy(bytes).into_owned())
                })
                .collect::<Vec<Box<str>>>()
        };

        let entities = unsafe {
            core::slice::from_raw_parts(d.entities.data, d.entities.len as usize).to_vec()
        };

        let blocks =
            unsafe { core::slice::from_raw_parts(d.blocks.data, d.blocks.len as usize).to_vec() };

        let params =
            unsafe { core::slice::from_raw_parts(d.params.data, d.params.len as usize).to_vec() };

        let points =
            unsafe { core::slice::from_raw_parts(d.points.data, d.points.len as usize).to_vec() };

        let brushes =
            unsafe { core::slice::from_raw_parts(d.brushes.data, d.brushes.len as usize).to_vec() };

        let line_weights = unsafe {
            core::slice::from_raw_parts(d.line_weights.data, d.line_weights.len as usize).to_vec()
        };

        let first_drawing_entity = d.first_drawing_entity;
        let world_coordinate_space = d.world_coordinate_space;

        Self {
            strings,
            entities,
            blocks,
            params,
            points,
            brushes,
            line_weights,
            first_drawing_entity,
            world_coordinate_space,
        }
    }
}
/// Debug print a [`CompatDrawing`].
#[unsafe(no_mangle)]
pub extern "C" fn debug_print_compat_drawing(d: &CompatDrawing) {
    println!("{:#?}", Drawing::from(d));
}

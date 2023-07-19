// Copyright 2013-2015 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::os::raw::c_void;

use base::{CFRange, CFIndex, CFAllocatorRef, CFTypeID, Boolean, CFComparisonResult};
use string::CFStringRef;

pub type CFArrayRetainCallBack = extern "C" fn(allocator: CFAllocatorRef, value: *const c_void) -> *const c_void;
pub type CFArrayReleaseCallBack = extern "C" fn(allocator: CFAllocatorRef, value: *const c_void);
pub type CFArrayCopyDescriptionCallBack = extern "C" fn(value: *const c_void) -> CFStringRef;
pub type CFArrayEqualCallBack = extern "C" fn(value1: *const c_void, value2: *const c_void) -> Boolean;
pub type CFArrayApplierFunction = extern "C" fn(value: *const c_void, context: *mut c_void);
pub type CFComparatorFunction = extern "C" fn(val1: *const c_void, val2: *const c_void) -> CFComparisonResult;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFArrayCallBacks {
    pub version: CFIndex,
    pub retain: CFArrayRetainCallBack,
    pub release: CFArrayReleaseCallBack,
    pub copyDescription: CFArrayCopyDescriptionCallBack,
    pub equal: CFArrayEqualCallBack,
}

#[repr(C)]
pub struct __CFArray(c_void);

pub type CFArrayRef = *const __CFArray;
pub type CFMutableArrayRef = *mut __CFArray;

extern {
    /*
     * CFArray.h
     */
    pub static kCFTypeArrayCallBacks: CFArrayCallBacks;

    pub fn CFArrayGetTypeID() -> CFTypeID;
    pub fn CFArrayCreate(allocator: CFAllocatorRef, values: *const *const c_void, numValues: CFIndex, callBacks: *const CFArrayCallBacks) -> CFArrayRef;
    pub fn CFArrayCreateCopy(allocator: CFAllocatorRef , theArray: CFArrayRef) -> CFArrayRef;
    pub fn CFArrayCreateMutable(allocator: CFAllocatorRef, capacity: CFIndex, callBacks: *const CFArrayCallBacks) -> CFMutableArrayRef;
    pub fn CFArrayCreateMutableCopy(allocator: CFAllocatorRef, capacity: CFIndex, theArray: CFArrayRef)-> CFMutableArrayRef;
    pub fn CFArrayGetCount(theArray: CFArrayRef) -> CFIndex;
    pub fn CFArrayGetCountOfValue(theArray: CFArrayRef, range: CFRange, value: *const c_void) -> CFIndex;
    pub fn CFArrayContainsValue(theArray: CFArrayRef, range: CFRange, value: *const c_void) -> Boolean;
    pub fn CFArrayGetValueAtIndex(theArray: CFArrayRef, idx: CFIndex) -> *const c_void;
    pub fn CFArrayGetValues(theArray: CFArrayRef, range: CFRange, values: *mut *const c_void);
    pub fn CFArrayApplyFunction(theArray: CFArrayRef, range: CFRange, applier: CFArrayApplierFunction, context: *mut c_void);
    pub fn CFArrayGetFirstIndexOfValue(theArray: CFArrayRef, range: CFRange, value: *const c_void) -> CFIndex;
    pub fn CFArrayGetLastIndexOfValue(theArray: CFArrayRef, range: CFRange, value: *const c_void) -> CFIndex;
    pub fn CFArrayBSearchValues(theArray: CFArrayRef, range: CFRange, value: *const c_void, comparator: CFComparatorFunction, context: *mut c_void) -> CFIndex;
    pub fn CFArrayAppendValue(theArray: CFMutableArrayRef, value: *const c_void);
    pub fn CFArrayInsertValueAtIndex(theArray: CFMutableArrayRef, idx: CFIndex, value: *const c_void);
    pub fn CFArraySetValueAtIndex(theArray: CFMutableArrayRef, idx: CFIndex, value: *const c_void);
    pub fn CFArrayRemoveValueAtIndex(theArray: CFMutableArrayRef, idx: CFIndex);
    pub fn CFArrayRemoveAllValues(theArray: CFMutableArrayRef);
    pub fn CFArrayReplaceValues(theArray: CFMutableArrayRef, range: CFRange, newValues: *mut *const c_void, newCount: CFIndex);
    pub fn CFArrayExchangeValuesAtIndices(theArray: CFMutableArrayRef, idx1: CFIndex, idx2: CFIndex);
    pub fn CFArraySortValues(theArray: CFMutableArrayRef, range: CFRange, comparator: CFComparatorFunction, context: *mut c_void);
    pub fn CFArrayAppendArray(theArray: CFMutableArrayRef, otherArray: CFArrayRef, otherRange: CFRange );
}

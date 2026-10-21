//! Сырые привязки к CoreGraphics, CoreFoundation и Accessibility (AX).
//! Свои, а не из крейта: нужно десятка два функций, и так точно видно,
//! что именно зовём.

#![allow(non_upper_case_globals, dead_code)]

use std::ffi::c_void;

pub type CFTypeRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CGEventRef = *mut c_void;
pub type CFMachPortRef = *mut c_void;
pub type CFRunLoopSourceRef = *mut c_void;
pub type CFRunLoopRef = *mut c_void;
pub type AXUIElementRef = *const c_void;

pub type TapCallback = unsafe extern "C" fn(
    proxy: *mut c_void,
    kind: u32,
    event: CGEventRef,
    info: *mut c_void,
) -> CGEventRef;

// Типы событий (CGEventType).
pub const kCGEventNull: u32 = 0;
pub const kCGEventLeftMouseDown: u32 = 1;
pub const kCGEventRightMouseDown: u32 = 3;
pub const kCGEventKeyDown: u32 = 10;
pub const kCGEventKeyUp: u32 = 11;
pub const kCGEventFlagsChanged: u32 = 12;
pub const kCGEventOtherMouseDown: u32 = 25;
pub const kCGEventTapDisabledByTimeout: u32 = 0xFFFF_FFFE;
pub const kCGEventTapDisabledByUserInput: u32 = 0xFFFF_FFFF;

// Поля события (CGEventField).
pub const kCGKeyboardEventAutorepeat: u32 = 8;
pub const kCGKeyboardEventKeycode: u32 = 9;
pub const kCGEventTargetUnixProcessID: u32 = 41;
pub const kCGEventSourceUserData: u32 = 42;

// Флаги модификаторов (CGEventFlags).
pub const kCGEventFlagMaskShift: u64 = 0x0002_0000;
pub const kCGEventFlagMaskControl: u64 = 0x0004_0000;
pub const kCGEventFlagMaskAlternate: u64 = 0x0008_0000;
pub const kCGEventFlagMaskCommand: u64 = 0x0010_0000;

// Куда ставить перехват и куда слать события.
pub const kCGHIDEventTap: u32 = 0;
pub const kCGSessionEventTap: u32 = 1;
pub const kCGHeadInsertEventTap: u32 = 0;
pub const kCGEventTapOptionDefault: u32 = 0;

// Состояние источника событий для «что зажато сейчас».
pub const kCGEventSourceStateHIDSystemState: i32 = 1;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub static kCFRunLoopCommonModes: CFStringRef;
    pub static kCFBooleanTrue: CFTypeRef;
    pub static kCFTypeDictionaryKeyCallBacks: c_void;
    pub static kCFTypeDictionaryValueCallBacks: c_void;

    pub fn CFMachPortCreateRunLoopSource(
        alloc: *const c_void,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    pub fn CFRunLoopRun();
    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFDictionaryCreate(
        alloc: *const c_void,
        keys: *const CFTypeRef,
        values: *const CFTypeRef,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> CFTypeRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        info: *mut c_void,
    ) -> CFMachPortRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    pub fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    pub fn CGEventGetFlags(event: CGEventRef) -> u64;
    pub fn CGEventSetFlags(event: CGEventRef, flags: u64);
    pub fn CGEventSetType(event: CGEventRef, kind: u32);
    pub fn CGEventKeyboardGetUnicodeString(
        event: CGEventRef,
        max: usize,
        actual: *mut usize,
        buf: *mut u16,
    );
    pub fn CGEventKeyboardSetUnicodeString(event: CGEventRef, len: usize, buf: *const u16);
    pub fn CGEventCreateKeyboardEvent(
        source: *const c_void,
        keycode: u16,
        down: bool,
    ) -> CGEventRef;
    pub fn CGEventPost(tap: u32, event: CGEventRef);
    pub fn CGEventSourceFlagsState(state: i32) -> u64;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub static kAXTrustedCheckOptionPrompt: CFStringRef;
    pub fn AXIsProcessTrusted() -> bool;
    pub fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
    pub fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
}

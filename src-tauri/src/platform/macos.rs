use super::{report, CapturePolicy, Click, InputRegion, PointerPhase, Status};
use std::{
    ffi::c_void,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
};

type Ref = *mut c_void;
#[repr(C)]
struct Point {
    x: f64,
    y: f64,
}
type Callback = unsafe extern "C" fn(Ref, u32, Ref, Ref) -> Ref;
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: Ref) -> bool;
    static kAXTrustedCheckOptionPrompt: Ref;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: Callback,
        info: Ref,
    ) -> Ref;
    fn CGEventTapEnable(tap: Ref, enabled: bool);
    fn CGEventTapIsEnabled(tap: Ref) -> bool;
    fn CGEventGetFlags(event: Ref) -> u64;
    fn CGEventGetLocation(event: Ref) -> Point;
    fn CGEventCreate(source: Ref) -> Ref;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFBooleanTrue: Ref;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
    fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> Ref;
    static kCFRunLoopDefaultMode: Ref;
    fn CFMachPortCreateRunLoopSource(allocator: Ref, port: Ref, order: isize) -> Ref;
    fn CFRunLoopGetCurrent() -> Ref;
    fn CFRunLoopAddSource(loop_ref: Ref, source: Ref, mode: Ref);
    fn CFRunLoopRemoveSource(loop_ref: Ref, source: Ref, mode: Ref);
    fn CFRunLoopRunInMode(mode: Ref, seconds: f64, return_after_source: bool) -> i32;
    fn CFMachPortInvalidate(port: Ref);
    fn CFRelease(object: Ref);
}

pub fn permission() -> bool {
    input_permission() && accessibility_permission()
}
pub fn input_permission() -> bool {
    unsafe { CGPreflightListenEventAccess() }
}
pub fn accessibility_permission() -> bool {
    unsafe { AXIsProcessTrusted() }
}
pub fn request_permission() -> bool {
    unsafe {
        if !input_permission() {
            CGRequestListenEventAccess();
        }
        if !accessibility_permission() {
            let key = kAXTrustedCheckOptionPrompt;
            let value = kCFBooleanTrue;
            let options = CFDictionaryCreate(
                ptr::null_mut(),
                &key,
                &value,
                1,
                ptr::addr_of!(kCFTypeDictionaryKeyCallBacks).cast(),
                ptr::addr_of!(kCFTypeDictionaryValueCallBacks).cast(),
            );
            if !options.is_null() {
                AXIsProcessTrustedWithOptions(options);
                CFRelease(options);
            }
        }
    }
    permission()
}
pub fn cursor() -> Option<Click> {
    unsafe {
        let event = CGEventCreate(ptr::null_mut());
        if event.is_null() {
            return None;
        }
        let p = CGEventGetLocation(event);
        CFRelease(event);
        Some(Click { x: p.x, y: p.y })
    }
}
struct Context {
    sender: SyncSender<super::Trigger>,
    tap: Ref,
    policy: CapturePolicy,
    region: InputRegion,
}

unsafe extern "C" fn callback(_: Ref, kind: u32, event: Ref, info: Ref) -> Ref {
    if info.is_null() {
        return event;
    }
    let context = &mut *(info as *mut Context);
    // 超时回调可能没有事件对象；先恢复事件端口，再处理普通事件。
    if kind == 0xffff_fffe || kind == 0xffff_ffff {
        CGEventTapEnable(context.tap, true);
        return event;
    }
    if event.is_null() {
        return event;
    }
    let flags = CGEventGetFlags(event);
    let required = (1 << 19) | (1 << 17); // Option + Shift
    if kind == 1 {
        super::count(0);
        if flags & (1 << 19) != 0 {
            super::count(1);
        }
        if flags & (1 << 17) != 0 {
            super::count(2);
        }
    }
    let modifiers = flags & required == required && flags & ((1 << 18) | (1 << 20)) == 0;
    let location = CGEventGetLocation(event);
    let point = Click {
        x: location.x,
        y: location.y,
    };
    let phase = match kind {
        1 => PointerPhase::Down,
        2 => PointerPhase::Up,
        6 => PointerPhase::Drag,
        _ => PointerPhase::Other,
    };
    let decision = context
        .policy
        .handle(phase, modifiers, context.region.contains(point));
    if decision.trigger {
        super::count(3);
        let _ = context.sender.try_send(super::Trigger::new(point));
    }
    if decision.consume {
        ptr::null_mut()
    } else {
        event
    }
}

pub fn run(
    sender: SyncSender<super::Trigger>,
    stop: Arc<AtomicBool>,
    status: Status,
    region: InputRegion,
) {
    unsafe {
        let mut context = Box::new(Context {
            sender,
            tap: ptr::null_mut(),
            policy: CapturePolicy::default(),
            region,
        });
        // 可过滤的 session tap，只接收左键事件，不监听键入内容。
        let tap = CGEventTapCreate(
            1,
            0,
            0,
            (1 << 1) | (1 << 2) | (1 << 6),
            callback,
            (&mut *context as *mut Context).cast(),
        );
        if tap.is_null() {
            report(&status, "无法启动点击拦截，请检查输入监控与辅助功能权限");
            return;
        }
        context.tap = tap;
        let source = CFMachPortCreateRunLoopSource(ptr::null_mut(), tap, 0);
        if source.is_null() {
            CFMachPortInvalidate(tap);
            CFRelease(tap);
            report(&status, "无法建立输入事件循环");
            return;
        }
        let loop_ref = CFRunLoopGetCurrent();
        CFRunLoopAddSource(loop_ref, source, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, true);
        report(&status, "监听中");
        while !stop.load(Ordering::Relaxed) {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, false);
            if !permission() || !CGEventTapIsEnabled(tap) {
                report(&status, "输入权限或监听状态已改变，请重新启用");
                break;
            }
        }
        CGEventTapEnable(tap, false);
        CFRunLoopRemoveSource(loop_ref, source, kCFRunLoopDefaultMode);
        CFMachPortInvalidate(tap);
        CFRelease(source);
        CFRelease(tap);
    }
    if stop.load(Ordering::Relaxed) {
        report(&status, "已暂停");
    }
}

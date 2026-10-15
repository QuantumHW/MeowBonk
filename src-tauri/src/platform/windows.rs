use super::{report, CapturePolicy, Click, InputRegion, PointerPhase, Status};
use std::{
    cell::RefCell,
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
};
use windows_sys::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::*,
    },
};

struct Context {
    sender: SyncSender<super::Trigger>,
    policy: CapturePolicy,
    region: InputRegion,
}
thread_local! { static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) }; }
pub fn input_permission() -> bool {
    true
}
pub fn accessibility_permission() -> bool {
    true
}
pub fn permission() -> bool {
    true
}
pub fn request_permission() -> bool {
    true
}
pub fn cursor() -> Option<Click> {
    unsafe {
        let mut point = std::mem::zeroed();
        (GetCursorPos(&mut point) != 0).then_some(Click {
            x: point.x as f64,
            y: point.y as f64,
        })
    }
}
unsafe extern "system" fn callback(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 && (w == WM_LBUTTONDOWN as usize || w == WM_LBUTTONUP as usize) {
        let event = &*(l as *const MSLLHOOKSTRUCT);
        let down = |key: u16| GetAsyncKeyState(key as i32) < 0;
        let alt = down(VK_MENU);
        let shift = down(VK_SHIFT);
        let modifiers = alt
            && shift
            && !down(VK_CONTROL)
            && !down(VK_LWIN)
            && !down(VK_RWIN)
            && event.flags & LLMHF_INJECTED == 0;
        let point = Click {
            x: event.pt.x as f64,
            y: event.pt.y as f64,
        };
        let phase = if w == WM_LBUTTONDOWN as usize {
            super::count(0);
            if alt {
                super::count(1);
            }
            if shift {
                super::count(2);
            }
            PointerPhase::Down
        } else {
            PointerPhase::Up
        };
        let consume = CONTEXT.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(context) = slot.as_mut() {
                let decision =
                    context
                        .policy
                        .handle(phase, modifiers, context.region.contains(point));
                if decision.trigger {
                    super::count(3);
                    let _ = context.sender.try_send(super::Trigger::new(point));
                }
                decision.consume
            } else {
                false
            }
        });
        if consume {
            return 1;
        }
    }
    CallNextHookEx(ptr::null_mut(), code, w, l)
}
pub fn run(
    sender: SyncSender<super::Trigger>,
    stop: Arc<AtomicBool>,
    status: Status,
    region: InputRegion,
) {
    unsafe {
        CONTEXT.with(|slot| {
            *slot.borrow_mut() = Some(Context {
                sender,
                policy: CapturePolicy::default(),
                region,
            })
        });
        let hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(callback),
            GetModuleHandleW(ptr::null()),
            0,
        );
        if hook.is_null() {
            CONTEXT.with(|slot| *slot.borrow_mut() = None);
            report(&status, "无法建立鼠标监听");
            return;
        }
        report(&status, "监听中");
        let mut message: MSG = std::mem::zeroed();
        while !stop.load(Ordering::Relaxed) {
            while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            MsgWaitForMultipleObjects(0, ptr::null(), 0, 250, QS_ALLINPUT);
        }
        UnhookWindowsHookEx(hook);
        CONTEXT.with(|slot| *slot.borrow_mut() = None);
        report(&status, "已暂停");
    }
}

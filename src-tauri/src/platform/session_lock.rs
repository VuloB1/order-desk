//! Detecta el bloqueo de la sesión de Windows (WM_WTSSESSION_CHANGE) con una
//! ventana oculta propia, ya que Tauri no expone ese evento.
use std::sync::mpsc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

const WTS_SESSION_LOGOFF: usize = 0x6;
const WTS_SESSION_LOCK: usize = 0x7;

type Callback = Box<dyn Fn() + Send>;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE && matches!(wparam.0, WTS_SESSION_LOCK | WTS_SESSION_LOGOFF) {
        let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Callback;
        if !ptr.is_null() {
            unsafe { (*ptr)() };
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Lanza un hilo con una ventana oculta que llama a `on_lock` al bloquearse la
/// sesión. Devuelve el HWND (como isize) para poder simular el mensaje en tests.
pub fn watch(on_lock: impl Fn() + Send + 'static) -> windows::core::Result<isize> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || unsafe {
        let setup = || -> windows::core::Result<HWND> {
            let hinst = GetModuleHandleW(None)?;
            let class = w!("OrderDeskSessionWatcher");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: hinst.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            // Ventana de nivel superior nunca mostrada (las message-only no reciben WTS de forma fiable).
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                None,
                None,
                Some(hinst.into()),
                None,
            )?;
            let cb: Box<Callback> = Box::new(Box::new(on_lock));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(cb) as isize);
            WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)?;
            Ok(hwnd)
        };
        let res = setup();
        let _ = tx.send(res.as_ref().map(|h| h.0 as isize).map_err(|e| e.clone()));
        if res.is_ok() {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
    rx.recv()
        .unwrap_or_else(|_| Err(windows::Win32::Foundation::E_FAIL.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // Simula el mensaje del SO: valida el cableado, no la entrega real de Windows.
    #[test]
    fn lock_message_triggers_callback() {
        let (tx, rx) = mpsc::channel();
        let hwnd = watch(move || tx.send(()).unwrap()).expect("registro WTS");
        unsafe {
            PostMessageW(
                Some(HWND(hwnd as _)),
                WM_WTSSESSION_CHANGE,
                WPARAM(WTS_SESSION_LOCK),
                LPARAM(1),
            )
            .unwrap();
        }
        assert!(rx.recv_timeout(Duration::from_secs(3)).is_ok());
    }

    #[test]
    fn unlock_message_is_ignored() {
        let (tx, rx) = mpsc::channel();
        let hwnd = watch(move || tx.send(()).unwrap()).expect("registro WTS");
        unsafe {
            PostMessageW(
                Some(HWND(hwnd as _)),
                WM_WTSSESSION_CHANGE,
                WPARAM(0x8),
                LPARAM(1),
            )
            .unwrap();
        }
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    }
}

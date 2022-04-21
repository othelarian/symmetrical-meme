use std::{
  cell::RefCell,
  ffi::{CString, OsStr},
  os::windows::ffi::OsStrExt,
  ptr::{null, null_mut},
  sync::mpsc::channel,
  thread
};
use tokio::sync::mpsc::UnboundedSender;
use winapi::{
  ctypes::{c_ulong, c_ushort},
  shared::{
    self,
    minwindef::{DWORD, HINSTANCE, LPARAM, LRESULT, UINT, WPARAM},
    ntdef::LPCWSTR,
    windef::{HBITMAP, HBRUSH, HICON, HMENU, HWND, POINT}
  },
  um::{
    errhandlingapi::GetLastError,
    shellapi::{
      self, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_MODIFY,
      NOTIFYICONDATAW, ShellExecuteA
    },
    winuser::{
      self, CW_USEDEFAULT, MENUINFO, MENUITEMINFOW, MIIM_FTYPE, MIIM_ID,
      MIIM_STATE, MIIM_STRING, MIM_APPLYTOSUBMENUS, MIM_STYLE, MFT_STRING,
      MNS_NOTIFYBYPOS, WNDCLASSW, WS_OVERLAPPEDWINDOW
    }
  }
};

thread_local!(
  static WININFO_STASH: RefCell<Option<WindowsLoopData>> = RefCell::new(None)
);


unsafe fn get_err(msg: &str) -> String {
  format!("{}: {}", &msg, GetLastError())
}

pub fn url_open() {
  let cmd = CString::new("open").unwrap();
  let location = CString::new("http://localhost:3030").unwrap();
  unsafe {
    ShellExecuteA(
      null_mut(), cmd.as_ptr(), location.as_ptr(), null(), null(),
      winuser::SW_SHOWNORMAL
    );
  }
}

fn to_wstring(str: &str) -> Vec<u16> {
  OsStr::new(str).encode_wide().chain(Some(0).into_iter()).collect::<Vec<_>>()
}

#[derive(Clone)]
struct WindowInfo {
  pub hmenu: HMENU,
  pub hwnd: HWND
}

unsafe impl Send for WindowInfo {}
unsafe impl Sync for WindowInfo {}

#[derive(Clone)]
struct WindowsLoopData {
  info: WindowInfo,
  q_tx: UnboundedSender<()>
}

unsafe extern "system" fn window_proc(
  h_wnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM
) -> LRESULT {
  let _wm_user_inc = winuser::WM_USER + 1;
  match msg {
    winuser::WM_MENUCOMMAND => WININFO_STASH.with(|stash| {
      let stash = stash.borrow();
      if let Some(stash) = stash.as_ref() {
        let menu_id = winuser::GetMenuItemID(stash.info.hmenu, w_param as i32);
        if (menu_id as i32) != -1 {
          match menu_id {
            1 => url_open(),
            2 => {
              stash.q_tx.send(()).unwrap();
              winuser::PostMessageW(
                stash.info.hwnd, winuser::WM_DESTROY,
                0 as WPARAM, 0 as LPARAM
              );
            }
            _ => ()
          }
        }
      }
    }),
    winuser::WM_DESTROY => winuser::PostQuitMessage(0),
    _wm_user_inc => {
      let lp = l_param as UINT;
      if lp == winuser::WM_LBUTTONUP || lp == winuser::WM_RBUTTONUP {
        let mut p = POINT { x: 0, y: 0 };
        if winuser::GetCursorPos(&mut p as *mut POINT) == 0 {
          return 1;
        }
        winuser::SetForegroundWindow(h_wnd);
        WININFO_STASH.with(|stash| {
          let stash = stash.borrow();
          let stash = stash.as_ref();
          if let Some(stash) = stash {
            winuser::TrackPopupMenu(
              stash.info.hmenu,
              0, p.x, p.y,
              (winuser::TPM_BOTTOMALIGN | winuser::TPM_LEFTALIGN) as i32,
              h_wnd,
              null_mut()
            );
          }
        });
      }
    }
  }
  winuser::DefWindowProcW(h_wnd, msg, w_param, l_param)
}

fn get_nid_struct(hwnd: &HWND) -> NOTIFYICONDATAW {
  NOTIFYICONDATAW {
    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as DWORD,
    hWnd: *hwnd,
    uID: 0x1 as UINT,
    uFlags: 0 as UINT,
    hBalloonIcon: 0 as HICON,
    uCallbackMessage: 0 as UINT,
    hIcon: 0 as HICON,
    szTip: [0 as u16; 128],
    dwState: 0 as DWORD,
    dwStateMask: 0 as DWORD,
    szInfo: [0 as u16; 256],
    u: Default::default(),
    szInfoTitle: [0 as u16; 64],
    dwInfoFlags: 0 as UINT,
    guidItem: shared::guiddef::GUID {
      Data1: 0 as c_ulong, Data2: 0 as c_ushort,
      Data3: 0 as c_ushort, Data4: [0; 8]
    }
  }
}

fn get_menu_item_struct() -> MENUITEMINFOW {
  let z_ui = 0 as UINT;
  MENUITEMINFOW {
    cbSize: std::mem::size_of::<MENUITEMINFOW>() as UINT,
    fMask: z_ui, fType: z_ui, fState: z_ui, wID: z_ui,
    hSubMenu: 0 as HMENU,
    hbmpChecked: 0 as HBITMAP,
    hbmpUnchecked: 0 as HBITMAP,
    dwItemData: 0 as shared::basetsd::ULONG_PTR,
    dwTypeData: null_mut(),
    cch: 0 as u32,
    hbmpItem: 0 as HBITMAP
  }
}

unsafe fn init_window() -> Result<WindowInfo, String> {
  let class_name = to_wstring("sys_win");
  let z_inst = 0 as HINSTANCE;
  let idi_app = winuser::IDI_APPLICATION;
  let wnd = WNDCLASSW {
    style: 0,
    lpfnWndProc: Some(window_proc),
    cbClsExtra: 0,
    cbWndExtra: 0,
    hInstance: z_inst,
    hIcon: winuser::LoadIconW(z_inst, idi_app),
    hCursor: winuser::LoadCursorW(z_inst, idi_app),
    hbrBackground: 16 as HBRUSH,
    lpszMenuName: 0 as LPCWSTR,
    lpszClassName: class_name.as_ptr()
  };
  if winuser::RegisterClassW(&wnd) == 0 {
    Err(get_err("Error creating window class"))
  } else {
    let hwnd = winuser::CreateWindowExW(
      0,
      class_name.as_ptr(),
      to_wstring("rust_systray_win").as_ptr(),
      WS_OVERLAPPEDWINDOW,
      CW_USEDEFAULT,
      0,
      CW_USEDEFAULT,
      0,
      0 as HWND,
      0 as HMENU,
      z_inst,
      null_mut()
    );
    if hwnd == null_mut() { Err(get_err("Error creating window")) }
    else {
      let mut nid = get_nid_struct(&hwnd);
      nid.uFlags = NIF_MESSAGE;
      nid.uCallbackMessage = winuser::WM_USER + 1;
      if shellapi::Shell_NotifyIconW(
        NIM_ADD, &mut nid as *mut NOTIFYICONDATAW
      ) == 0 { Err(get_err("Error adding menu icon")) } else {
        let hmenu = winuser::CreatePopupMenu();
        let mut m = MENUINFO {
          cbSize: std::mem::size_of::<MENUINFO>() as DWORD,
          fMask: MIM_APPLYTOSUBMENUS | MIM_STYLE,
          dwStyle: MNS_NOTIFYBYPOS,
          cyMax: 0 as UINT,
          hbrBack: 0 as HBRUSH,
          dwContextHelpID: 0 as DWORD,
          dwMenuData: 0 as shared::basetsd::ULONG_PTR
        };
        if winuser::SetMenuInfo(hmenu, &mut m as *const MENUINFO) == 0 {
          Err(get_err("Error setting up menu"))
        } else { Ok(WindowInfo { hmenu, hwnd }) }
      }
    }
  }
}

unsafe fn run_loop() {
  let mut msg = winuser::MSG {
    hwnd: 0 as HWND,
    message: 0 as UINT,
    wParam: 0 as WPARAM,
    lParam: 0 as LPARAM,
    time: 0 as DWORD,
    pt: POINT {x: 0, y: 0}
  };
  loop {
    winuser::GetMessageW(&mut msg, 0 as HWND, 0, 0);
    if msg.message == winuser::WM_QUIT { break; }
    winuser::TranslateMessage(&mut msg);
    winuser::DispatchMessageW(&mut msg);
  }
}

struct Window { info: WindowInfo }

impl Window {
  fn new(q_tx: UnboundedSender<()>) -> Result<Self, String> {
    let (tx, rx) = channel();
    thread::spawn(move || unsafe {
      let i = init_window();
      let k;
      match i {
        Ok(j) => { tx.send(Ok(j.clone())).ok(); k = j; }
        Err(e) => { tx.send(Err(e)).ok(); return; }
      };
      WININFO_STASH.with(|stash| {
        let data = WindowsLoopData { info: k, q_tx };
        (*stash.borrow_mut()) = Some(data);
      });
      run_loop();
    });
    rx.recv().unwrap().and_then(|info| {
      Ok(Self { info })
    })
  }
}

pub struct Tray { window: Window }

impl Tray {
  pub fn new(q_tx: UnboundedSender<()>) -> Result<Self, String> {
    match Window::new(q_tx) {
      Ok(window) => {
        let tray = Self { window };
        tray.set_icon()
          .and(tray.set_tooltip())
          .and(tray.add_menu_entry(1, "Open the webpage"))
          .and(tray.add_menu_entry(2, "Quit"))
          .and(Ok(tray))
      }
      Err(e) => Err(e)
    }
  }

  fn set_icon(&self) -> Result<(), String> {
    let wstr_path = to_wstring("./logo_64.ico");
    let hicon;
    unsafe {
      hicon = winuser::LoadImageW(
        null_mut() as HINSTANCE, wstr_path.as_ptr(),
        winuser::IMAGE_ICON, 64, 64, winuser::LR_LOADFROMFILE
      ) as HICON;
    }
    if hicon == null_mut() as HICON {
      Err(unsafe { get_err("Error loading image") })
    } else {
      let mut nid = get_nid_struct(&self.window.info.hwnd);
      nid.uFlags = NIF_ICON;
      nid.hIcon = hicon;
      unsafe {
        if shellapi::Shell_NotifyIconW(
          NIM_MODIFY, &mut nid as *mut NOTIFYICONDATAW
        ) == 0 { Err(get_err("Error setting icon")) } else { Ok(()) }
      }
    }
  }

  fn set_tooltip(&self) -> Result<(), String> {
    let tt = "Symmetrical-Meme".as_bytes();
    let mut nid = get_nid_struct(&self.window.info.hwnd);
    for i in 0..tt.len() { nid.szTip[i] = tt[i] as u16; }
    nid.uFlags = NIF_TIP;
    unsafe {
      if shellapi::Shell_NotifyIconW(
        NIM_MODIFY, &mut nid as *mut NOTIFYICONDATAW
      ) == 0 { Err(get_err("Error setting tooltip")) } else { Ok(()) }
    }
  }

  fn add_menu_entry(&self, id: u32, name: &str) -> Result<(), String> {
    let mut wstr_name = to_wstring(name);
    let mut item = get_menu_item_struct();
     item.fMask = MIIM_FTYPE | MIIM_STRING | MIIM_ID | MIIM_STATE;
     item.fType = MFT_STRING;
     item.wID = id;
     item.dwTypeData = wstr_name.as_mut_ptr();
     item.cch = (name.len() * 2) as u32;
     unsafe {
       if winuser::InsertMenuItemW(
         self.window.info.hmenu, id, 1, &item as *const MENUITEMINFOW
        ) == 0 { Err(get_err("Error inserting menu item")) } else { Ok(()) }
     }
  }
}

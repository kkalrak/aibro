use cef::*;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    sync::mpsc,
    time::{Duration, Instant},
};
pub type Reply = mpsc::Sender<Result<Value, String>>;
pub struct Work {
    pub op: UiOp,
    pub reply: Reply,
    pub deadline: Instant,
}
pub enum UiOp {
    List,
    Open(String),
    Close(i32),
    Cdp(i32, String, Value),
    Shutdown,
}
struct Pending {
    reply: Reply,
    deadline: Instant,
}
#[derive(Default)]
struct State {
    browsers: BTreeMap<i32, Browser>,
    registrations: BTreeMap<i32, Registration>,
    pending: BTreeMap<i32, Pending>,
    sequence: i32,
    quit: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
wrap_dev_tools_message_observer! {
    struct Observer;
    impl DevToolsMessageObserver {
        fn on_dev_tools_method_result(&self, _browser: Option<&mut Browser>, message_id: i32, success: i32, result: Option<&[u8]>) {
            let pending = STATE.with(|s| s.borrow_mut().pending.remove(&message_id));
            if let Some(p) = pending {
                let value = serde_json::from_slice(result.unwrap_or(b"{}"));
                let answer = if success != 0 { value.map_err(|e| e.to_string()) }
                    else { Err(format!("CDP: {}",String::from_utf8_lossy(result.unwrap_or(b"unknown error")))) };
                let _ = p.reply.send(answer);
            }
        }
    }
}
wrap_life_span_handler! {
    struct Life;
    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(b) = browser {
                let mut observer = Observer::new();
                let registration = b.host().and_then(|h| h.add_dev_tools_message_observer(Some(&mut observer)));
                STATE.with(|s| { let mut s=s.borrow_mut(); s.browsers.insert(b.identifier(),b.clone());
                    if let Some(r)=registration { s.registrations.insert(b.identifier(),r); } });
            }
        }
        fn on_before_close(&self, browser: Option<&mut Browser>) {
            if let Some(b)=browser { STATE.with(|s| { let mut s=s.borrow_mut();
                s.browsers.remove(&b.identifier()); s.registrations.remove(&b.identifier());
                if s.browsers.is_empty() { s.quit=true; } }); }
        }
    }
}
wrap_client! {
    struct AgentClient;
    impl Client { fn life_span_handler(&self) -> Option<LifeSpanHandler> { Some(Life::new()) } }
}
fn open(url: &str) -> Result<Value, String> {
    let mut client = AgentClient::new();
    let window = WindowInfo {
        runtime_style: RuntimeStyle::CHROME,
        ..Default::default()
    }
    .set_as_popup(Default::default(), "Aibro");
    let b = browser_host_create_browser_sync(
        Some(&window),
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        None,
    )
    .ok_or("CEF could not create browser")?;
    Ok(json!({"tab":b.identifier()}))
}
wrap_browser_process_handler! {
    struct Process;
    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            let url=std::env::args().find_map(|a| a.strip_prefix("--url=").map(str::to_owned))
                .unwrap_or_else(|| "https://example.com".into());
            if let Err(e)=open(&url) { eprintln!("{e}"); STATE.with(|s| s.borrow_mut().quit=true); }
        }
        fn default_client(&self) -> Option<Client> { Some(AgentClient::new()) }
    }
}
wrap_app! {
    struct AgentApp;
    impl App { fn browser_process_handler(&self)->Option<BrowserProcessHandler> { Some(Process::new()) } }
}
fn browser(id: i32) -> Result<Browser, String> {
    STATE
        .with(|s| s.borrow().browsers.get(&id).cloned())
        .ok_or_else(|| format!("Unknown tab {id}; call tabs"))
}
fn dispatch(work: Work) {
    if Instant::now() > work.deadline {
        let _ = work
            .reply
            .send(Err("Command expired before execution".into()));
        return;
    }
    let result=match work.op {
        UiOp::List=>Ok(STATE.with(|s| json!(s.borrow().browsers.iter().map(|(id,b)| json!({"tab":id,
            "url":b.main_frame().map(|f| CefString::from(&f.url()).to_string()),"loading":b.is_loading()!=0
        })).collect::<Vec<_>>()))),
        UiOp::Open(url)=>open(&url),
        UiOp::Shutdown=> {
            let browsers=STATE.with(|s| s.borrow().browsers.values().cloned().collect::<Vec<_>>());
            for b in browsers { if let Some(h)=b.host() { h.close_browser(1); } }
            Ok(json!({"closing":true}))
        }
        UiOp::Close(id)=>browser(id).and_then(|b| { b.host().ok_or("No browser host")?.close_browser(1); Ok(json!({"closing":id})) }),
        UiOp::Cdp(id,method,params)=>match browser(id).and_then(|b| b.host().ok_or_else(|| "No browser host".into())) {
            Err(e)=>Err(e),
            Ok(host)=> {
                let seq=STATE.with(|s| { let mut s=s.borrow_mut(); s.sequence+=1; let id=s.sequence;
                    s.pending.insert(id,Pending {reply:work.reply.clone(),deadline:work.deadline}); id });
                let message=json!({"id":seq,"method":method,"params":params}).to_string();
                if host.send_dev_tools_message(Some(message.as_bytes()))!=0 { return; }
                STATE.with(|s| s.borrow_mut().pending.remove(&seq));
                Err("CEF rejected DevTools command".into())
            }
        }
    };
    let _ = work.reply.send(result);
}
pub fn run(main_args: MainArgs, sandbox: *mut u8) -> anyhow::Result<()> {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let mut app = AgentApp::new();
    if execute_process(Some(&main_args), Some(&mut app), sandbox) >= 0 {
        return Ok(());
    }
    let root = std::env::args()
        .find_map(|a| {
            a.strip_prefix("--aibro-data-dir=")
                .map(std::ffi::OsString::from)
        })
        .or_else(|| std::env::var_os("AIBRO_DATA_DIR"))
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_exe()?.parent().unwrap().join(".aibro"));
    std::fs::create_dir_all(&root)?;
    let settings = Settings {
        root_cache_path: CefString::from(root.join("profile").to_string_lossy().as_ref()),
        log_file: CefString::from(root.join("chromium.log").to_string_lossy().as_ref()),
        ..Default::default()
    };
    anyhow::ensure!(
        initialize(Some(&main_args), Some(&settings), Some(&mut app), sandbox) != 0,
        "CEF initialization failed"
    );
    let (tx, rx) = mpsc::channel();
    let service = crate::api::start(tx, &root)?;
    loop {
        do_message_loop_work();
        for work in rx.try_iter().take(32) {
            dispatch(work);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let now = Instant::now();
            s.pending.retain(|_, p| {
                if p.deadline < now {
                    let _ = p.reply.send(Err("CDP timed out".into()));
                    false
                } else {
                    true
                }
            });
        });
        if STATE.with(|s| s.borrow().quit) {
            break;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    service.stop();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for (_, p) in std::mem::take(&mut s.pending) {
            let _ = p.reply.send(Err("Browser closed".into()));
        }
        s.registrations.clear();
        s.browsers.clear();
    });
    shutdown();
    Ok(())
}

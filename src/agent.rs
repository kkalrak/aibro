use crate::runtime::{UiOp, Work};
use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::mpsc,
    time::{Duration, Instant},
};
#[derive(Deserialize, Debug)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Tabs,
    Open {
        url: String,
    },
    Navigate {
        tab: i32,
        url: String,
    },
    Snapshot {
        tab: i32,
    },
    Click {
        tab: i32,
        target: String,
    },
    Fill {
        tab: i32,
        target: String,
        text: String,
    },
    Select {
        tab: i32,
        target: String,
        value: String,
    },
    Press {
        tab: i32,
        key: String,
    },
    Scroll {
        tab: i32,
        #[serde(default)]
        x: i32,
        y: i32,
    },
    Screenshot {
        tab: i32,
    },
    Back {
        tab: i32,
    },
    Reload {
        tab: i32,
    },
    Close {
        tab: i32,
    },
    Shutdown,
}
pub struct Agent {
    sender: mpsc::Sender<Work>,
    contexts: HashMap<i32, i64>,
}
impl Agent {
    pub fn new(sender: mpsc::Sender<Work>) -> Self {
        Self {
            sender,
            contexts: HashMap::new(),
        }
    }
    fn ui(&self, op: UiOp) -> Result<Value> {
        let (tx, rx) = mpsc::channel();
        self.sender.send(Work {
            op,
            reply: tx,
            deadline: Instant::now() + Duration::from_secs(15),
        })?;
        rx.recv_timeout(Duration::from_secs(16))
            .context("Browser did not respond")?
            .map_err(|e| anyhow!(e))
    }
    fn cdp(&self, tab: i32, method: &str, params: Value) -> Result<Value> {
        self.ui(UiOp::Cdp(tab, method.into(), params))
    }
    fn evaluate(&self, tab: i32, context: i64, expression: String) -> Result<Value> {
        let result=self.cdp(tab,"Runtime.evaluate",json!({"expression":expression,"contextId":context,"returnByValue":true,"awaitPromise":true}))?;
        if !result["exceptionDetails"].is_null() {
            bail!("Page operation failed: {}", result["exceptionDetails"])
        }
        Ok(result["result"]["value"].clone())
    }
    fn settle(&self, tab: i32) -> Result<()> {
        let start = Instant::now();
        loop {
            let tabs = self.ui(UiOp::List)?;
            let item = tabs
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["tab"] == tab)
                .context("Tab was closed")?;
            if item["loading"] == false {
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(10) {
                bail!("Page still loading; call snapshot to inspect current state")
            }
            std::thread::sleep(Duration::from_millis(80));
        }
    }
    fn snapshot(&mut self, tab: i32) -> Result<Value> {
        let tree = self.cdp(tab, "Page.getFrameTree", json!({}))?;
        let frame = tree["frameTree"]["frame"]["id"]
            .as_str()
            .context("No main frame")?;
        let world = self.cdp(
            tab,
            "Page.createIsolatedWorld",
            json!({"frameId":frame,"worldName":"aibro-agent"}),
        )?;
        let context = world["executionContextId"]
            .as_i64()
            .context("No isolated execution context")?;
        self.contexts.insert(tab, context);
        self.evaluate(
            tab,
            context,
            include_str!("js/snapshot.js").replace(
                "__SNAPSHOT_TOKEN__",
                &serde_json::to_string(&uuid::Uuid::new_v4().to_string())?,
            ),
        )
    }
    fn target(&self, tab: i32, target: &str, body: &str) -> Result<Value> {
        let context = *self
            .contexts
            .get(&tab)
            .context("Call snapshot before using a target reference")?;
        let target = serde_json::to_string(target)?;
        self.evaluate(tab,context,format!(r#"(()=>{{const id={target};const s=globalThis.__aibro;
            const el=s?.nodes.get(id);if(!el||!el.isConnected)throw Error('Stale reference; take a new snapshot');
            if(el.disabled||el.getAttribute('aria-disabled')==='true')throw Error('Target is disabled');
            el.scrollIntoView({{block:'center',inline:'center'}});{body}}})()"#))
    }
    pub fn execute(&mut self, command: Command) -> Result<Value> {
        match command {
            Command::Tabs => self.ui(UiOp::List),
            Command::Open { url } => {
                validate_url(&url)?;
                let v = self.ui(UiOp::Open(url))?;
                let tab = v["tab"].as_i64().context("No tab id")? as i32;
                self.settle(tab)?;
                Ok(json!({"tab":tab,"page":self.snapshot(tab)?}))
            }
            Command::Navigate { tab, url } => {
                validate_url(&url)?;
                self.contexts.remove(&tab);
                let result = self.cdp(tab, "Page.navigate", json!({"url":url}))?;
                if let Some(error) = result["errorText"].as_str() {
                    bail!("Navigation failed: {error}")
                }
                self.settle(tab)?;
                self.snapshot(tab)
            }
            Command::Snapshot { tab } => self.snapshot(tab),
            Command::Click { tab, target } => {
                let point=self.target(tab,&target,r#"const r=el.getBoundingClientRect();const x=Math.max(0,r.left)+Math.min(r.width,innerWidth-Math.max(0,r.left))/2;
                    const y=Math.max(0,r.top)+Math.min(r.height,innerHeight-Math.max(0,r.top))/2;
                    let hit=el.getRootNode().elementFromPoint(x,y);if(hit!==el&&!el.contains(hit))throw Error('Target is covered or outside viewport');return {x,y};"#)?;
                let x = point["x"].as_f64().context("No target x")?;
                let y = point["y"].as_f64().context("No target y")?;
                self.cdp(
                    tab,
                    "Input.dispatchMouseEvent",
                    json!({"type":"mouseMoved","x":x,"y":y}),
                )?;
                for kind in ["mousePressed", "mouseReleased"] {
                    self.cdp(
                        tab,
                        "Input.dispatchMouseEvent",
                        json!({"type":kind,"x":x,"y":y,"button":"left","clickCount":1}),
                    )?;
                }
                self.settle(tab)?;
                self.snapshot(tab)
            }
            Command::Fill { tab, target, text } => {
                self.target(tab,&target,r#"if(el.readOnly)throw Error('Target is read only');
                    if(!(el instanceof HTMLInputElement||el instanceof HTMLTextAreaElement||el.isContentEditable))throw Error('Target is not editable');
                    if(el instanceof HTMLInputElement&&!['text','search','email','url','tel','password'].includes(el.type))throw Error('Input type is not supported by fill');
                    el.focus();if(el.isContentEditable){const r=document.createRange();r.selectNodeContents(el);const s=getSelection();s.removeAllRanges();s.addRange(r)}else{el.select()}return true;"#)?;
                self.cdp(tab, "Input.insertText", json!({"text":text}))?;
                self.snapshot(tab)
            }
            Command::Select { tab, target, value } => {
                let value = serde_json::to_string(&value)?;
                self.target(tab,&target,&format!("if(!(el instanceof HTMLSelectElement))throw Error('Target is not a select');const v={value};if(!Array.from(el.options).some(o=>o.value===v&&!o.disabled))throw Error('Unknown or disabled option');el.value=v;el.dispatchEvent(new Event('input',{{bubbles:true}}));el.dispatchEvent(new Event('change',{{bubbles:true}}));return true;"))?;
                self.snapshot(tab)
            }
            Command::Press { tab, key } => {
                let code = match key.as_str() {
                    "Enter" => 13,
                    "Tab" => 9,
                    "Escape" => 27,
                    "Backspace" => 8,
                    "Delete" => 46,
                    "ArrowLeft" => 37,
                    "ArrowUp" => 38,
                    "ArrowRight" => 39,
                    "ArrowDown" => 40,
                    _ => bail!("Unsupported key"),
                };
                self.cdp(tab,"Input.dispatchKeyEvent",json!({"type":"keyDown","key":key,"windowsVirtualKeyCode":code,"text":if key=="Enter" {"\r"} else {""}}))?;
                self.cdp(
                    tab,
                    "Input.dispatchKeyEvent",
                    json!({"type":"keyUp","key":key,"windowsVirtualKeyCode":code}),
                )?;
                self.settle(tab)?;
                self.snapshot(tab)
            }
            Command::Scroll { tab, x, y } => {
                self.cdp(
                    tab,
                    "Input.dispatchMouseEvent",
                    json!({"type":"mouseWheel","x":400,"y":300,"deltaX":x,"deltaY":y}),
                )?;
                std::thread::sleep(Duration::from_millis(120));
                self.snapshot(tab)
            }
            Command::Screenshot { tab } => {
                let v = self.cdp(tab, "Page.captureScreenshot", json!({"format":"png"}))?;
                Ok(json!({"mime":"image/png","base64":v["data"]}))
            }
            Command::Back { tab } => {
                let h = self.cdp(tab, "Page.getNavigationHistory", json!({}))?;
                let index = h["currentIndex"].as_i64().unwrap_or(0);
                if index < 1 {
                    bail!("No previous history entry")
                }
                self.cdp(
                    tab,
                    "Page.navigateToHistoryEntry",
                    json!({"entryId":h["entries"][(index-1) as usize]["id"]}),
                )?;
                self.contexts.remove(&tab);
                self.settle(tab)?;
                self.snapshot(tab)
            }
            Command::Reload { tab } => {
                self.contexts.remove(&tab);
                self.cdp(tab, "Page.reload", json!({}))?;
                self.settle(tab)?;
                self.snapshot(tab)
            }
            Command::Close { tab } => {
                self.contexts.remove(&tab);
                self.ui(UiOp::Close(tab))
            }
            Command::Shutdown => self.ui(UiOp::Shutdown),
        }
    }
}
fn validate_url(url: &str) -> Result<()> {
    if !(url.starts_with("http://") || url.starts_with("https://") || url == "about:blank") {
        bail!("Only http://, https:// and about:blank URLs are supported")
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_ambiguous_commands() {
        assert!(
            serde_json::from_str::<Command>(r#"{"op":"click","tab":1,"target":"e1","extra":true}"#)
                .is_err()
        );
    }
    #[test]
    fn rejects_executable_urls() {
        for u in [
            "javascript:alert(1)",
            "file:///C:/secret",
            "data:text/html,test",
        ] {
            assert!(validate_url(u).is_err())
        }
    }
}

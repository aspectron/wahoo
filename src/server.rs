use crate::prelude::*;
use async_std::prelude::*;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::sync::Mutex;
use std::{collections::HashMap, time::Duration};
use tide_websockets::{Message, WebSocket};
use workflow_core::id::Id;

pub struct Server {
    // pub tide : tide::Server<()>,
    port: u16,
    location: Option<String>,
    paths: Vec<PathBuf>,
    // update : Receiver<()>,
    websockets: Arc<Mutex<HashMap<Id, Arc<tide_websockets::WebSocketConnection>>>>,
}

impl Server {
    pub fn new(port: u16, location: Option<String>, paths: &[PathBuf]) -> Arc<Server> {
        let server = Self {
            // tide: tide::new(),
            port,
            location,
            paths: paths.to_vec(),
            websockets: Arc::new(Mutex::new(HashMap::new())),
        };

        Arc::new(server)
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        // setup debouncer
        let (tx, rx) = std::sync::mpsc::channel();

        // No specific tickrate, max debounce time 2 seconds
        let mut debouncer = new_debouncer(Duration::from_secs(1), None, tx).unwrap();

        let watcher = debouncer.watcher();
        for path in self.paths.iter() {
            println!("Watching {}", path.to_str().unwrap());
            watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;
        }

        log_info!("Server", "monitoring changes...",);

        let this = self.clone();
        tokio::spawn(async move {
            match this.http_server().await {
                Ok(_) => {}
                Err(e) => {
                    log_error!("Server Error: {:?}", e);
                }
            }
        });

        for _events in rx {
            let ctx = Arc::new(Context::create(self.location.clone(), Options::default()).await?);
            let build = Arc::new(Builder::new(ctx));
            build.execute().await?;
            self.post("update").await?;
            log_info!("HTTP", "server listening on port {}", self.port);
            log_info!("Server", "monitoring changes...",);
        }
        Ok(())
    }

    async fn post(&self, msg: &str) -> Result<()> {
        let websockets = self
            .websockets
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();

        for websocket in websockets {
            websocket.send(Message::Text(msg.to_string())).await.ok();
        }

        Ok(())
    }

    async fn http_server(self: Arc<Self>) -> Result<()> {
        let mut app = tide::new();
        app.with(tide::log::LogMiddleware::new());
        app.at("/").serve_dir("site/")?;
        app.at("/").serve_file("site/index.html")?;

        let this = self.clone();
        let websockets = this.websockets.clone();
        app.at("/wahoo")
            .get(WebSocket::new(move |_request, mut stream| {
                let websockets = websockets.clone();
                async move {
                    let id = Id::new();

                    websockets
                        .clone()
                        .lock()
                        .unwrap()
                        .insert(id, Arc::new(stream.clone()));

                    while let Some(Ok(Message::Text(_input))) = stream.next().await {
                        // let output: String = input.chars().rev().collect();
                        // stream
                        //     .send_string(format!("{} | {}", &input, &output))
                        //     .await?;
                    }

                    websockets.clone().lock().unwrap().remove(&id);

                    Ok(())
                }
            }));

        let address = format!("127.0.0.1:{}", self.port);
        log_info!("HTTP", "server listening on {address}");
        app.listen(address).await?;

        Ok(())
    }
}

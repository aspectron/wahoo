use crate::prelude::*;
use async_std::prelude::*;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::sync::Mutex;
use std::{collections::HashMap, time::Duration};
use tide_websockets::{Message, WebSocket};
use workflow_core::id::Id;
// use std::hash::BuildHasher;
use ahash::RandomState;

use serde::Serialize;
// #[derive(Debug, Serialize)]
// struct UpdateNotification {
//     files: Vec<String>,
// }

// #[derive(Debug, Serialize)]
// struct Session {
//     id : String
// }


#[derive(Debug, Serialize)]
struct Notification<Msg> where Msg : Serialize {
    method : String,
    params : Msg
}


fn notification<P>(method: &str, params : P) -> String where P : Serialize {
    let notification = Notification {
        method : method.to_string(), params
    };

    serde_json::to_string(&notification).unwrap()
}

pub struct Server {
    // ctx : Arc<Context>,
    // pub tide : tide::Server<()>,
    port: u16,
    location: Option<String>,
    project_folder: PathBuf,
    site_folder: PathBuf,
    paths: Vec<PathBuf>,
    settings: Settings,
    websockets: Arc<Mutex<HashMap<Id, Arc<tide_websockets::WebSocketConnection>>>>,
    session : Id,
    hashes : Mutex<HashMap<String, u64>>,
}

impl Server {
    pub fn new(
        // ctx : &Arc<Context>,
        port: u16,
        location: Option<String>,
        project_folder: PathBuf,
        site_folder: PathBuf,
        paths: &[PathBuf],
        settings: Settings,
    ) -> Arc<Server> {
        let server = Self {
            // ctx : ctx.clone(),
            port,
            location,
            project_folder,
            site_folder,
            paths: paths.to_vec(),
            websockets: Arc::new(Mutex::new(HashMap::new())),
            settings,
            session: Id::new(),
            hashes : Mutex::new(HashMap::new()),
        };

        Arc::new(server)
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        // setup debouncer
        let (tx, rx) = std::sync::mpsc::channel();

        // No specific tickrate, max debounce time 2 seconds
        let mut debouncer = new_debouncer(Duration::from_millis(500), None, tx).unwrap();

        let watcher = debouncer.watcher();
        for path in self.paths.iter() {
            log_info!("Watching", "{}", path.to_str().unwrap());
            watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;
        }

        // let base_folder = self.ctx.mani

        // log_info!("Server", "monitoring changes...",);

        let this = self.clone();
        tokio::spawn(async move {
            match this.http_server().await {
                Ok(_) => {}
                Err(e) => {
                    log_error!("Server Error: {:?}", e);
                }
            }
        });

        for (_index, events) in rx.iter().enumerate() {
            // log_info!("", "");
            if events.is_err() {
                continue;
            }
            log_info!("Event", "events: {:?}", events);

            // Having this here, causes multiple refreshes when saving style.css
            // let ctx = Arc::new(Context::create(self.location.clone(), Options { server : true }).await?);
            // let build = Arc::new(Builder::new(ctx));
            // build.execute().await?;

            let files: Vec<String> = events
                .unwrap()
                .iter()
                .filter_map(|event| {
                    let f = event.path.strip_prefix(&self.project_folder).expect("watched file is not in the project folder");
                    let file_str = f.as_os_str().to_str().unwrap().to_string();

                    let hash_builder = RandomState::with_seed(42);
                    let content = std::fs::read_to_string(&event.path).ok();
                    let hash = hash_builder.hash_one(content);
                    if let Some(prev_hash) = self.hashes.lock().unwrap().insert(file_str.clone(), hash) {
                        if hash == prev_hash {
                            // println!("Skipping {}", file_str);
                            return None;
                        }
                    }

                    if file_str.contains("templates/") {
                        let parts = file_str.split("templates/").collect::<Vec<_>>();
                        if parts.len() == 2 {
                        // parts.next();
                            parts.last().map(|v|v.to_string())
                        } else {
                            None
                        }
                    } else {
                        Some(file_str)
                    }
                })
                .collect();

            if !files.is_empty() {
                
                let ctx = Arc::new(Context::create(self.location.clone(), Options { server : true }).await?);
                let build = Arc::new(Builder::new(ctx));
                build.execute().await?;

                // let noti = UpdateNotification { files }; 
                // let str = serde_json::to_string(&noti)?;
                let update = notification("update", files);
                // log_info!("Notification", "{}", update);
                self.post(&update).await?;
                log_info!("HTTP", "server listening on port {}", self.port);
                // log_info!("Server", "monitoring changes...",);
            }
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

    // fn target_folder(&self, path: &str) -> PathBuf {
    //     self.target_folder.join(path)
    // }

    async fn http_server(self: Arc<Self>) -> Result<()> {
        let mut app = tide::new();
        app.with(tide::log::LogMiddleware::new());
        app.at("/")
            .serve_dir(&self.site_folder)
            .map_err(|err| -> Error {
                format!(
                    "Unable to locate target folder `{}`: {err}",
                    self.site_folder.display()
                )
                .into()
            })?;
        if let Some(languages) = &self.settings.languages {
            if languages.contains(&"en".to_string()) {
                let target = self.site_folder.join("en/index.html");
                app.at("/").serve_file(&target).map_err(|err| -> Error {
                    format!("Unable to locate `{}`: {err}", target.display()).into()
                })?;
            } else if !languages.is_empty() {
                let locale = &languages[0];
                let target = self.site_folder.join(format!("{locale}/index.html"));
                app.at("/").serve_file(&target).map_err(|err| -> Error {
                    format!("Unable to locate `{}`: {err}", target.display()).into()
                })?;
            } else {
                let target = self.site_folder.join("index.html");
                app.at("/").serve_file(&target).map_err(|err| -> Error {
                    format!(
                        "Unable to locate target folder `{}`: {err}",
                        target.display()
                    )
                    .into()
                })?;
            }
        } else {
            let target = self.site_folder.join("index.html");
            app.at("/").serve_file(&target).map_err(|err| -> Error {
                format!(
                    "Unable to locate target folder `{}`: {err}",
                    target.display()
                )
                .into()
            })?;
        }

        let this = self.clone();
        let websockets = this.websockets.clone();

        app.at("/wahoo")
            .get(WebSocket::new(move |_request, mut stream| {
                let websockets = websockets.clone();
                let session = this.session.clone();
                async move {
                    let id = Id::new();

                    websockets
                        .clone()
                        .lock()
                        .unwrap()
                        .insert(id, Arc::new(stream.clone()));

                    // let session = Session { id : session.to_string() };
                    stream.send(Message::Text(notification("session", session.to_string()))).await.ok();


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
        log_info!("HTTP", "serving `{}`", self.site_folder.display());
        app.listen(address).await?;

        Ok(())
    }
}

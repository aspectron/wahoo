use crate::prelude::*;
use async_std::prelude::*;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::sync::Mutex;
use std::{collections::HashMap, time::Duration};
use tide_websockets::{Message, WebSocket};
use workflow_core::id::Id;
// use std::hash::BuildHasher;
use tide::http::mime;
//use tide::utils::After;
use tide::utils::async_trait;
use tide::Middleware;
use tide::{Response, Result, StatusCode};

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
struct Notification<Msg>
where
    Msg: Serialize,
{
    id: String,
    method: String,
    params: Msg,
}

fn notification<P>(method: &str, params: P) -> String
where
    P: Serialize,
{
    let notification = Notification {
        id: Id::new().to_string(),
        method: method.to_string(),
        params,
    };

    serde_json::to_string(&notification).unwrap()
}

pub struct Server {
    // ctx : Arc<Context>,
    // pub tide : tide::Server<()>,
    // verbose: bool,
    port: u16,
    location: Option<String>,
    project_folder: PathBuf,
    site_folder: PathBuf,
    watch_targets: Vec<PathBuf>,
    settings: Settings,
    websockets: Arc<Mutex<HashMap<Id, Arc<tide_websockets::WebSocketConnection>>>>,
    session: Id,
    hashes: Mutex<HashMap<String, u64>>,
    sink: Sink,
    // verbose: bool,
}

impl Server {
    pub fn new(
        // ctx : &Arc<Context>,
        port: u16,
        location: Option<String>,
        project_folder: PathBuf,
        site_folder: PathBuf,
        watch_targets: &[PathBuf],
        settings: Settings,
        sink: Sink,
        // verbose : bool,
    ) -> Arc<Server> {
        let server = Self {
            // ctx : ctx.clone(),
            port,
            location,
            project_folder,
            site_folder,
            watch_targets: watch_targets.to_vec(),
            websockets: Arc::new(Mutex::new(HashMap::new())),
            settings,
            session: Id::new(),
            hashes: Mutex::new(HashMap::new()),
            sink,
            // verbose
        };

        Arc::new(server)
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        // setup debouncer
        let (tx, rx) = std::sync::mpsc::channel();

        // No specific tickrate, max debounce time 2 seconds
        let mut debouncer = new_debouncer(Duration::from_millis(500), None, tx).unwrap();

        let watcher = debouncer.watcher();
        for path in self.watch_targets.iter() {
            log_trace!("Watching", "{}", style(path.to_str().unwrap()).cyan());
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
            // log_info!("Event", "events: {:?}", events);

            // Having this here, causes multiple refreshes when saving style.css
            // let ctx = Arc::new(Context::create(self.location.clone(), Options { server : true }).await?);
            // let build = Arc::new(Builder::new(ctx));
            // build.execute().await?;

            let files: Vec<String> = events
                .unwrap()
                .iter()
                .filter_map(|event| {
                    let f = event
                        .path
                        .strip_prefix(&self.project_folder)
                        .expect("watched file is not in the project folder");
                    let file_str = f.as_os_str().to_str().unwrap().to_string();

                    match std::fs::read(&event.path) {
                        Ok(content) => {
                            let hash = make_hash(&content);
                            if let Some(prev_hash) =
                                self.hashes.lock().unwrap().insert(file_str.clone(), hash)
                            {
                                if hash == prev_hash {
                                    // println!("Skipping {}", file_str);
                                    return None;
                                }
                            }

                            if file_str.contains("templates/") {
                                let parts = file_str.split("templates/").collect::<Vec<_>>();
                                if parts.len() == 2 {
                                    // parts.next();
                                    parts.last().map(|v| v.to_string())
                                } else {
                                    None
                                }
                            } else {
                                Some(file_str)
                            }
                        }
                        Err(err) => {
                            log_error!("Unable to read `{}`: {}", event.path.display(), err);
                            None
                        }
                    }
                })
                .collect();

            if !files.is_empty() {
                let ctx = Arc::new(
                    Context::create(
                        self.location.clone(),
                        Options {
                            server: true,
                            ..Options::default()
                        },
                    )
                    .await?,
                );
                let site_folder = ctx.site_folder.clone();
                let build = Arc::new(Builder::new_with_sink(ctx, self.sink.clone()));
                build.execute().await?;

                // let noti = UpdateNotification { files };
                // let str = serde_json::to_string(&noti)?;
                let update = notification("update", files);
                // log_info!("Notification", "{}", update);
                self.post(&update).await?;

                let update_json_file = site_folder.join("__wahoo.json");
                std::fs::write(update_json_file, update).ok();

                log_trace!("HTTP", "server listening on port {}", self.port);
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

        let empty_list = vec![];
        let languages = self.settings.languages.as_ref().unwrap_or(&empty_list);
        if languages.is_empty() {
            let target = self.site_folder.join("index.html");
            app.at("/").serve_file(&target).map_err(|err| -> Error {
                format!(
                    "Unable to locate target folder `{}`: {err}",
                    target.display()
                )
                .into()
            })?;
        } else {
            let root_locale = if languages.contains(&"en".to_string()) {
                "en"
            } else {
                languages[0].as_str()
            };

            let target = self.site_folder.join(format!("{root_locale}/index.html"));
            app.at("/").serve_file(&target).map_err(|err| -> Error {
                format!("Unable to locate `{}`: {err}", target.display()).into()
            })?;

            for locale in languages {
                let target = self.site_folder.join(format!("{locale}/index.html"));
                for path in [
                    format!("/{locale}").as_str(),
                    format!("/{locale}/").as_str(),
                ] {
                    app.at(path).serve_file(&target).map_err(|err| -> Error {
                        format!("Unable to locate `{}`: {err}", target.display()).into()
                    })?;
                }
            }
        }

        let this = self.clone();
        let websockets = this.websockets.clone();

        app.at("/wahoo")
            .get(WebSocket::new(move |_request, mut stream| {
                let websockets = websockets.clone();
                let session = this.session;
                async move {
                    let id = Id::new();

                    websockets
                        .clone()
                        .lock()
                        .unwrap()
                        .insert(id, Arc::new(stream.clone()));

                    // let session = Session { id : session.to_string() };
                    stream
                        .send(Message::Text(notification("session", session.to_string())))
                        .await
                        .ok();

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

        const NOT_FOUND_HTML_PAGE: &str = "<html>
            <body style=\"text-align:center;margin: 100px;\">
            <h1>Error 404</h1>
            <h2>uh oh, we couldn't find that document</h2>
          </body></html>";

        const INTERNAL_SERVER_ERROR_HTML_PAGE: &str = "<html>
            <body style=\"text-align:center;margin: 100px;\">
            <h1>Error 500</h1>
            <h2>whoops! it's not you, it's us</h2>
            <p>
              Please try again later.
            </p>
          </body></html>";

        pub struct ErrorHandler {
            site_folder: PathBuf,
            server: Arc<Server>,
            languages: Vec<String>,
        }

        impl ErrorHandler {
            async fn run(&self, response: Response, site_folder: PathBuf) -> tide::Result {
                let mut page_404 = NOT_FOUND_HTML_PAGE.to_string();
                let mut page_500 = INTERNAL_SERVER_ERROR_HTML_PAGE.to_string();

                let response = match response.status() {
                    StatusCode::NotFound => {
                        if let Some(error_content) = &self.server.settings.error_404 {
                            if error_content.ends_with(".html") {
                                let path = site_folder.join(error_content);
                                page_404 = fs::read_to_string(path).await?;
                            } else {
                                page_404 = error_content.clone();
                            }
                        }
                        Response::builder(404)
                            .content_type(mime::HTML)
                            .body(page_404.as_str())
                            .build()
                    }

                    StatusCode::InternalServerError => {
                        if let Some(error_content) = &self.server.settings.error_500 {
                            if error_content.ends_with(".html") {
                                let path = site_folder.join(error_content);
                                page_500 = fs::read_to_string(path).await?;
                            } else {
                                page_500 = error_content.clone();
                            }
                        }
                        Response::builder(500)
                            .content_type(mime::HTML)
                            .body(page_500.as_str())
                            .build()
                    }

                    _ => response,
                };

                Ok(response)
            }
        }

        #[async_trait]
        impl<State> Middleware<State> for ErrorHandler
        where
            State: Clone + Send + Sync + 'static,
        {
            async fn handle(
                &self,
                request: tide::Request<State>,
                next: tide::Next<'_, State>,
            ) -> tide::Result {
                let url = request.url();
                let site_folder = if let Some(mut path_segments) = url.path_segments() {
                    let locale = path_segments.next().unwrap().to_string();
                    if self.languages.contains(&locale) {
                        self.site_folder.join(locale)
                    } else {
                        self.site_folder.clone()
                    }
                } else {
                    self.site_folder.clone()
                };
                let response = next.run(request).await;
                self.run(response, site_folder).await
            }
        }

        app.with(ErrorHandler {
            site_folder: self.site_folder.clone(),
            server: self.clone(),
            languages: languages.clone(),
        });

        let address = format!("127.0.0.1:{}", self.port);
        log_info!("HTTP", "server listening on {address}");
        log_info!("HTTP", "serving `{}`", self.site_folder.display());
        app.listen(address).await?;

        Ok(())
    }
}

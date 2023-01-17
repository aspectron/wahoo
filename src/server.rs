use crate::prelude::*;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::time::Duration;

pub struct Server {
    // pub tide : tide::Server<()>,
    port: u16,
    location: Option<String>,
    paths: Vec<PathBuf>,
}

impl Server {
    pub fn new(port: u16, location: Option<String>, paths: &[PathBuf]) -> Arc<Server> {
        let server = Self {
            // tide: tide::new(),
            port,
            location,
            paths: paths.to_vec(),
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
            watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;
        }

        log_info!("Server", "monitoring changes...",);

        let port = self.port;
        tokio::spawn(async move {
            match http_server(port).await {
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
            log_info!("HTTP", "server listening on port {}", self.port);
            log_info!("Server", "monitoring changes...",);
        }
        Ok(())
    }
}

async fn http_server(port: u16) -> Result<()> {
    let mut app = tide::new();
    app.with(tide::log::LogMiddleware::new());
    app.at("/").serve_dir("site/")?;
    let address = format!("127.0.0.1:{}", port);
    log_info!("HTTP", "server listening on {address}");
    app.listen(address).await?;

    Ok(())
}

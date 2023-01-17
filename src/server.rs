use crate::prelude::*;
use async_std::prelude::*;
use notify::RecursiveMode;
use notify_debouncer_mini::{
    new_debouncer_opt,
    DebounceEventHandler,
    //new_debouncer,
    notify::{
        Watcher,
        RecommendedWatcher,
        EventHandler,
        Config,
        Result as NResult
    },
    DebounceEventResult
};
use std::{time::Duration, collections::HashMap};
use tide_websockets::{Message, WebSocket};
use async_std::channel::*;
use workflow_core::id::Id;
use std::sync::Mutex;

pub struct CustomWatcher{
    inner: RecommendedWatcher,
}

struct EventWatcher{
    server: Arc<Server>,
    inner: std::sync::mpsc::Sender<DebounceEventResult>
}

impl EventWatcher{
    fn new(server: Arc<Server>, inner: std::sync::mpsc::Sender<DebounceEventResult>)->Self{
        Self {
            server,
            inner
        }
    }
}


impl DebounceEventHandler for EventWatcher{
    fn handle_event(&mut self, event: DebounceEventResult) {
        if !self.server.is_building(){
            self.inner.handle_event(event)
        }
    }
}



impl Watcher for CustomWatcher {
    /// Create a new watcher.
    fn new<F: EventHandler>(mut event_handler: F, config: Config) -> NResult<Self> {
        let config = config.with_compare_contents(true);
        let inner = RecommendedWatcher::new(move |res: NResult<notify::Event>|{
            
            if let Ok(a) = &res{
                let kind = &a.kind;
                match kind{
                    notify::EventKind::Modify(c)=>{
                        match c{
                            notify::event::ModifyKind::Data(a)=>{
                                println!("event_handler Modify: {:?}, {:#?}", a, res);
                                event_handler.handle_event(res);
                            }
                            _=>{

                            }
                        }
                    }
                    _=>{

                    }
                }
                
            }
            
        }, config)?;

        Ok(Self{
            inner
        })
    }

    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> NResult<()> {
        self.inner.watch(path, recursive_mode)
    }

    fn unwatch(&mut self, path: &Path) -> NResult<()> {
        self.inner.unwatch(path)
    }

    fn configure(&mut self, config: Config) -> NResult<bool> {
        self.inner.configure(config)
    }

    fn kind() -> notify::WatcherKind {
        notify::WatcherKind::Fsevent
    }
}

pub struct Server {
    // pub tide : tide::Server<()>,
    port: u16,
    location: Option<String>,
    paths: Vec<PathBuf>,
    // update : Receiver<()>,
    websockets : Arc<Mutex<HashMap<Id,tide_websockets::WebSocketConnection>>>,
    building: Arc<Mutex<bool>>
}

impl Server {
    pub fn new(port: u16, location: Option<String>, paths: &[PathBuf]) -> Arc<Server> {
        let server = Self {
            // tide: tide::new(),
            port,
            location,
            paths: paths.to_vec(),
            websockets: Arc::new(Mutex::new(HashMap::new())),
            building: Arc::new(Mutex::new(false))
        };

        Arc::new(server)
    }
    fn building(&self, building: bool){
        *self.building.lock().unwrap() = building;
    }
    pub fn is_building(&self)->bool{
        *self.building.lock().unwrap()
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        // setup debouncer
        let (tx, rx) = std::sync::mpsc::channel();

        // No specific tickrate, max debounce time 2 seconds
        //let mut debouncer = new_debouncer(Duration::from_millis(1000), Some(Duration::from_millis(100)), tx).unwrap();
        let mut debouncer = new_debouncer_opt::<EventWatcher, CustomWatcher>(
            Duration::from_millis(1000), 
            Some(Duration::from_millis(999)),
            EventWatcher::new(self.clone(), tx),
            notify::Config::default()
        ).unwrap();

    
        let watcher = debouncer.watcher();
        for _path in self.paths.iter() {
            //println!("path: {:?}", path);
            //watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;
        }
        

        let path = Path::new("/Users/surindersingh/Documents/dev/as/flow/workflow-dev/wahoo/test/src/templates/index.html");
        watcher.watch(path, RecursiveMode::NonRecursive)?;


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
        //let mut rx = rx.iter();
        for (index, events) in rx.iter().enumerate() {
            log_info!("", "");
            log_info!("Event", "events: {}, {:?}", index, events);

            log_info!("Event", "building1: {}", self.is_building());
            //self.building(true);
            log_info!("Event", "building2: {}", self.is_building());
            
            
            let ctx = Arc::new(Context::create(self.location.clone(), Options::default()).await?);
            let build = Arc::new(Builder::new(ctx));
            build.execute().await?;
            log_info!("HTTP", "server listening on port {}", self.port);
            log_info!("Server", "monitoring changes...",);

            self.building(false);
        }
        println!("sssssss");
        Ok(())
    }
    
    async fn http_server(self: Arc<Self>) -> Result<()> {
        let mut app = tide::new();
        app.with(tide::log::LogMiddleware::new());
        app.at("/").serve_dir("site/")?;
        //app.at("/").serve_file("site/index.html")?;

        let this = self.clone();
        let websockets = this.websockets.clone();
        //let x = Arc::new(123);
        //let v = x.clone();
        let (sender,receiver) = unbounded::<tide_websockets::WebSocketConnection>();
        app.at("/wahoo")
            .get(WebSocket::new(move |_request, mut stream| {
                //let x = x.clone();
                let websockets = websockets.clone(); 
                async move{
                    // let s = stream.clone();
                    let id = Id::new();
     
                    websockets.clone().lock().unwrap().insert(id, stream.clone());
            
                    while let Some(Ok(Message::Text(input))) = stream.next().await {
                        let output: String = input.chars().rev().collect();
            
                        stream
                            .send_string(format!("{} | {}", &input, &output))
                            .await?;
                    }
            
                    Ok(())
                }
            })
        );
    
    
        let address = format!("127.0.0.1:{}", self.port);
        log_info!("HTTP", "server listening on {address}");
        app.listen(address).await?;
    
        Ok(())
    }
}


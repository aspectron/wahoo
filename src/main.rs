use clap::{Parser, Subcommand};
use console::style;

pub mod builder;
pub mod context;
pub mod error;
pub mod filter;
pub mod log;
pub mod manifest;
pub mod markdown;
pub mod prelude;
pub mod result;
pub mod server;
pub mod sink;
pub mod utils;

use prelude::*;
use server::Server;

// #[derive(Debug, Parser)]
// #[clap(name = "cargo")]
// #[clap(bin_name = "cargo")]
// #[clap(
//     setting = clap::AppSettings::DeriveDisplayOrder,
//     dont_collapse_args_in_usage = true,
// )]
// enum Cmd {
//     #[clap(name = "wahoo")]
//     #[clap(about, author, version)]
//     #[clap(
//         setting = clap::AppSettings::DeriveDisplayOrder,
//     )]
//     Args(Args),
// }

#[derive(Debug, Parser)] //clap::Args)]
#[clap(name = "wahoo")]
#[clap(about, author, version)]
#[clap(
    // setting = clap::AppSettings::DeriveDisplayOrder,
)]
struct Args {
    /// Location of the nw.toml manifest file
    #[clap(name = "manifest")]
    location: Option<String>,
    /// Action to execute (build,clean,init)
    #[clap(subcommand)]
    action: Action,
    /// Enable verbose mode
    #[clap(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Action {
    /// Render the site
    Build {},
    /// Serve the site via HTTP; Monitor and re-render if changed
    Serve {
        /// HTTP server host
        #[clap(long, default_value = "127.0.0.1")]
        host: String,

        /// HTTP port to listen on
        #[clap(long, default_value = "8080")]
        port: u16,
    },
    /// Delete the rendered site files
    Clean {},
    /// Create a basic site template (TODO)
    Init {},
    /// Publish the site (TODO)
    Publish {},
}

pub async fn async_main() -> Result<()> {
    let Args {
        location,
        action,
        verbose,
    } = Args::parse();

    if verbose {
        log::enable_verbose();
    }

    match action {
        Action::Build {} => {
            let ctx = Arc::new(Context::create(location, Options::default()).await?);
            let build = Arc::new(Builder::new(ctx.clone()));
            build.execute().await?;
            println!();

            // ~~~
            // println!("{:#?}", ctx.manifest);
        }
        Action::Clean {} => {
            let ctx = Arc::new(Context::create(location, Options::default()).await?);

            ctx.clean().await?;
        }
        Action::Serve { host, port } => {
            let sink = Sink::default();

            let ctx = {
                let ctx = Arc::new(
                    Context::create(
                        location.clone(),
                        Options {
                            server: true,
                            verbose: true,
                        },
                    )
                    .await?,
                );
                let build = Arc::new(Builder::new_with_sink(ctx.clone(), sink.clone()));
                build.execute().await?;
                ctx
            };

            let mut watch_targets = ctx.manifest.imports.clone();
            watch_targets.extend([ctx.manifest_toml.clone(), ctx.src_folder.clone()]);

            if let Some(Settings {
                watch: Some(watch), ..
            }) = &ctx.manifest.settings
            {
                for folder in watch.iter() {
                    let target_folder = ctx.project_folder.join(folder);
                    let watch_target = target_folder.canonicalize().unwrap_or_else(|err| {
                        format!(
                            "Unable to locate watch target `{}`: {err}",
                            target_folder.display()
                        )
                        .into()
                    });
                    watch_targets.push(watch_target);
                }
            }
            // println!("{:#?}", ctx.manifest.sections);
            if let Some(sections) = &ctx.manifest.sections {
                for (_name, section) in sections.iter() {
                    if let Some(SectionSettings {
                        folder: Some(folder),
                        ..
                    }) = &section.settings
                    {
                        let target_folder = ctx.project_folder.join(folder);
                        let watch_target = target_folder.canonicalize().unwrap_or_else(|err| {
                            format!(
                                "Unable to locate section folder `{}`: {err}",
                                target_folder.display()
                            )
                            .into()
                        });
                        watch_targets.push(watch_target);
                    }
                }
            }

            log_trace!("Watching", "{watch_targets:#?}");

            let server = Server::new(
                host,
                port,
                location,
                ctx.project_folder.clone(),
                ctx.site_folder.clone(),
                &watch_targets,
                ctx.settings(),
                sink,
            );

            server.run().await?;
        }
        Action::Init {} => {
            // // let arch = Architecture::default();
            // let folder : PathBuf = env::current_dir().unwrap().into();
            // let name = if let Some(name) = name {
            //     name
            // } else {
            //     folder.file_name().unwrap().to_str().unwrap().to_string()
            // };
            // // let name = name.as_ref().unwrap_or(folder.file_name().expect("").to_str().expect());
            // let options = init::Options {
            //     js, manifest, force
            // };
            // let mut project = init::Project::try_new(name, folder)?;

            // project.generate(options).await?;
        }
        Action::Publish {} => {}
    }

    Ok(())
}

// fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
//     let (mut tx, rx) = channel();

//     // Automatically select the best implementation for your platform.
//     // You can also access each implementation directly e.g. INotifyWatcher.
//     let watcher = RecommendedWatcher::new(move |res| {
//         futures::executor::block_on(async {
//             tx.send(res).await.unwrap();
//         })
//     }, Config::default())?;

//     Ok((watcher, rx))
// }

// async fn async_watch<P: AsRef<Path>>(path: P) -> notify::Result<()> {
//     let (mut watcher, mut rx) = async_watcher()?;

//     // Add a path to be watched. All files and directories at that path and
//     // below will be monitored for changes.
//     watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

//     while let Some(res) = rx.next().await {
//         match res {
//             Ok(event) => println!("changed: {:?}", event),
//             Err(e) => println!("watch error: {:?}", e),
//         }
//     }

//     Ok(())
// }

// #[async_std::main]
#[tokio::main]
async fn main() -> Result<()> {
    let result = async_main().await;
    match &result {
        Err(Error::Warning(warn)) => {
            println!("\nWarning: {}\n", style(warn).yellow())
        }
        // Err(err) => println!("\n{}\n",style(format!("{}", err)).red()),
        // Err(err) => println!("\n{}\n",err),
        Err(err) => {
            log_error!("{}", err);
            println!();
        }
        Ok(_) => {}
    };

    if result.is_err() {
        std::process::exit(1);
    }

    Ok(())
}

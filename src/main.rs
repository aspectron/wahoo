// use std::{sync::Arc, env};
// use async_std::path::PathBuf;
use clap::{Parser,Subcommand};
use console::style;

pub mod error;
pub mod result;
pub mod manifest;
pub mod context;
pub mod builder;
pub mod log;
pub mod utils;
pub mod prelude;

use prelude::*;

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
    setting = clap::AppSettings::DeriveDisplayOrder,
)]
struct Args {
    /// Location of the nw.toml manifest file
    #[clap(name = "manifest")]
    location: Option<String>,
    /// Action to execute (build,clean,init)
    #[clap(subcommand)]
    action : Action,
    /// Enable verbose mode
    #[clap(short, long)]
    verbose : bool,
}

#[derive(Subcommand, Debug)]
enum Action {
    Build {
    },
    Clean { 
    },
    /// Create NW package template
    Init {
    },
    Publish {
    },
}


pub async fn async_main() -> Result<()> {
    
    let Args {
        location,
        action,
        verbose,
    }= Args::parse();

    if verbose {
        log::enable_verbose();
    }

    match action {
        Action::Build {
        } => {

            let ctx = Arc::new(Context::create(
                location,
                Options::default(),
            ).await?);

            let build = Arc::new(Builder::new(ctx));
            build.execute().await?;
        },
        Action::Clean { 
        } => {
            let ctx = Arc::new(Context::create(
                location,
                Options::default()
            ).await?);

            ctx.clean().await?;

        },
        Action::Init {
        } => {
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

        },
        Action::Publish {
        } => {
        },
    }

    Ok(())
}

// #[async_std::main]
#[tokio::main]
async fn main() -> Result<()> {
    let result = async_main().await;
    match &result {
        Err(Error::Warning(warn)) => println!("\nWarning: {}\n",style(format!("{}", warn)).yellow()),
        // Err(err) => println!("\n{}\n",style(format!("{}", err)).red()),
        // Err(err) => println!("\n{}\n",err),
        Err(err) => {
            log_error!("{}",err);
            println!("");
        },
        Ok(_) => { }
    };

    if result.is_err() {
        std::process::exit(1);
    }
    
    Ok(())
}


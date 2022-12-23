use std::collections::HashSet;
use crate::prelude::*;

pub struct Builder {
    ctx : Arc<Context>,
}

impl Builder {
    pub fn new(ctx: Arc<Context>) -> Builder {
        Builder {
            ctx
        }
    }

    pub async fn execute(&self) -> Result<()> {

        self.ctx.clean().await?;
        self.ctx.ensure_folders().await?;

        log_info!("Render","loading templates");

        let tera = match tera::Tera::new(self.ctx.project_folder.join("**/*").to_str().unwrap()) {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {}", e);
                ::std::process::exit(1);
            }
        };

        let context = &tera::Context::from_serialize(&self.ctx.manifest.toml)?;
        // let mut context = tera::Context::new();
        // context.insert("username", &"Bob");
        // context.insert("numbers", &vec![1, 2, 3]);
        // context.insert("show_all", &false);
        // context.insert("bio", &"<script>alert('pwnd');</script>");

        log_info!("Render","processing folders");

        let mut folders = HashSet::new();
        for template in tera.get_template_names() {
            let target_file = self.ctx.target_folder.join(template);
            let folder = Path::new(&target_file);
            if let Some(parent) = folder.parent() {
                folders.insert(parent.to_path_buf());
            }
        }
    
        for folder in folders {
            std::fs::create_dir_all(self.ctx.target_folder.join(folder))?; 
        }

        log_info!("Render","rendering");
        for template in tera.get_template_names() {
            use std::error::Error;
            match tera.render(template, &context) {
                Ok(s) => {
                    let target_file = self.ctx.target_folder.join(template);
                    // println!("{}\n{:?}", template, s)
                    fs::write(target_file,&s).await?;
                },
                Err(e) => {
                    // println!("Error: {}", e);
                    let mut cause = e.source();
                    while let Some(e) = cause {
                        println!("");
                        log_error!("{}",e);
                        cause = e.source();
                    }

                    return Err(e.into());
                }
            };
        }

        log_info!("Render","done");
        println!("");

        Ok(())
    }
}


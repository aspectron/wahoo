use crate::prelude::*;

#[derive(Debug)]
pub struct Options {
}

impl Default for Options {
    fn default() -> Self {
        Options {
        }
    }
}

#[derive(Debug)]
pub struct Context {

    pub manifest : Manifest,
    pub target_folder : PathBuf,
    pub project_folder : PathBuf,
}

impl Context {
    pub async fn create(
        location : Option<String>,
        // output : Option<String>,
        _options: Options,
    ) -> Result<Context> {

        let manifest_toml = Manifest::locate(location).await?;
        log_info!("Manifest","`{}`",manifest_toml.to_str().unwrap());
        let manifest = Manifest::load(&manifest_toml).await?;
        let manifest_folder = manifest_toml.parent().unwrap().to_path_buf();

        let target_folder = manifest_folder.join("site");
        let project_folder = manifest_folder.join("src");
        log_info!("Project","`{}`",project_folder.to_str().unwrap());
        log_info!("Target","`{}`",target_folder.to_str().unwrap());

        let ctx = Context {
            manifest,
            target_folder,
            project_folder,
        };

        Ok(ctx)
    }

    pub async fn ensure_folders(&self) -> Result<()> {
        let folders = [
            &self.target_folder,
        ];
        for folder in folders {
            if !std::path::Path::new(folder).exists() {
                std::fs::create_dir_all(folder)?;
            }
        }

        Ok(())
    }

    pub async fn clean(&self) -> Result<()> {
        if self.target_folder.exists().await {
            // log_info!("Cleaning","`{}`",self.target_folder.display());
            async_std::fs::remove_dir_all(&self.target_folder).await?;
        }
        Ok(())
    }

}


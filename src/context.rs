use crate::prelude::*;

#[derive(Default, Debug)]
pub struct Options {}

#[derive(Debug)]
pub struct Context {
    pub manifest: Manifest,
    pub manifest_toml: PathBuf,
    pub target_folder: PathBuf,
    pub project_folder: PathBuf,
}

impl Context {
    pub async fn create(
        location: Option<String>,
        // output : Option<String>,
        _options: Options,
    ) -> Result<Context> {
        let manifest_toml = Manifest::locate(location).await?;
        log_info!("Manifest", "`{}`", manifest_toml.to_str().unwrap());
        let manifest = Manifest::load(&manifest_toml).await?;
        let manifest_folder = manifest_toml.parent().unwrap().to_path_buf();

        let target_folder = manifest_folder.join("site");
        let project_folder = manifest_folder.join("src");
        log_info!("Project", "`{}`", project_folder.to_str().unwrap());
        log_info!("Target", "`{}`", target_folder.to_str().unwrap());

        let ctx = Context {
            manifest,
            manifest_toml,
            target_folder,
            project_folder,
        };

        Ok(ctx)
    }

    pub async fn ensure_folders(&self) -> Result<()> {
        let folders = [&self.target_folder];
        for folder in folders {
            if !std::path::Path::new(folder).exists() {
                std::fs::create_dir_all(folder)?;
            }
        }

        Ok(())
    }

    pub async fn clean(&self) -> Result<()> {
        if self.target_folder.exists() {
            // log_info!("Cleaning","`{}`",self.target_folder.display());

            for entry in std::fs::read_dir(&self.target_folder)? {
                let path = entry?.path();
                if path.is_dir() {
                    async_std::fs::remove_dir_all(&path).await?;
                } else {
                    std::fs::remove_file(path)?;
                }
            }

            // async_std::fs::remove_dir_all(&self.target_folder).await?;
        }
        Ok(())
    }
}

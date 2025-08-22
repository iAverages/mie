use backblaze_b2_client::client::B2Client;
use backblaze_b2_client::definitions::shared::B2File;
use std::{env, error::Error, path::Path, sync::Arc};
use tokio::fs::File;

type DynamicResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug)]
pub struct UploadFile {
    pub path: String,
}

// TODO: setup parallel uploads again (or completely remove it?)
pub async fn upload_files<F>(
    client: Arc<B2Client>,
    bucket_id: Arc<str>,
    files: Vec<UploadFile>,
    _: Option<F>,
) -> DynamicResult<Vec<DynamicResult<B2File>>>
where
    F: Fn(&str, u64, u64, f32, u64, u64) + Send + Sync + 'static,
{
    let results = vec![];
    for file in files {
        let path = Path::new(&file.path);
        let open_file = File::open(&file.path).await?;
        let file_metadata = open_file.metadata().await?;
        let file_size = file_metadata.len();
        let file_path_prefix = env::var("B2_BUCKET_PATH_PREFIX").unwrap_or_else(|_| String::new());

        let file_name = match path.file_name() {
            Some(name) => file_path_prefix + "/" + &name.to_string_lossy(),
            None => return Err(Box::from("Given file path is a folder.")),
        };

        let upload = client
            .create_upload(
                open_file,
                file_name,
                bucket_id.clone().to_string(),
                None,
                file_size,
                None,
            )
            .await;
        upload.start().await?;
    }
    Ok(results)
}

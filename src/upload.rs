use std::{
    collections::HashMap,
    convert::Infallible,
    env,
    error::Error,
    fs::File,
    io::{Read, Seek},
    path::Path,
    sync::{
        atomic::{self, AtomicU64},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use backblaze_b2_client::structs::{
    B2BasicError, B2Client, B2File, B2FinishLargeFileBody, B2GetUploadUrlBody,
    B2StartLargeFileUploadBody, B2UploadFileHeaders, B2UploadPartHeaders,
};
use futures::TryStreamExt;
use nonzero_ext::nonzero;

use tokio::{
    io::AsyncReadExt,
    sync::Mutex,
    task::{AbortHandle, JoinHandle},
};
use tokio_util::codec::{BytesCodec, FramedRead};

const MEGABYTE: u32 = 1048576;
type DynamicResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug)]
pub struct UploadFile {
    pub path: String,
    pub extra_info: Option<HashMap<String, String>>,
}

pub async fn upload_files<F>(
    client: Arc<Mutex<B2Client>>,
    bucket_id: Arc<str>,
    files: Vec<UploadFile>,
    on_chunk: Option<F>,
) -> DynamicResult<Vec<DynamicResult<B2File>>>
where
    F: Fn(&str, u64, u64, f32, u64, u64) + Send + Sync + 'static,
{
    let callback = Arc::new(on_chunk);
    let mut join_handles: Vec<JoinHandle<DynamicResult<B2File>>> = vec![];
    let abort_handles: Arc<RwLock<Vec<AbortHandle>>> = Arc::new(RwLock::new(Vec::new()));

    for file in files {
        let client = client.clone();
        let callback = callback.clone();
        let bucket_id = bucket_id.clone();
        let open_file = File::open(&file.path)?;

        let file_metadata = open_file.metadata()?;

        let file_size = file_metadata.len();
        let task_abort_handles = abort_handles.clone();

        let join_handle = tokio::spawn(async move {
            const UPLOAD_SIZE_LIMIT: u64 = MEGABYTE as u64 * 200;
            const MAX_RETRIES: u64 = 50;
            let mut retry_count = 0;

            loop {
                let callback = callback.clone();

                let inner_file_path = file.path.clone();

                let extra_file_info = match file.extra_info.as_ref() {
                    Some(map) => {
                        let mut ret_map: HashMap<String, serde_json::Value> =
                            HashMap::with_capacity(map.capacity());

                        for (key, value) in map.iter() {
                            ret_map.insert(
                                key.clone(),
                                serde_json::to_value(urlencoding::encode(value))?,
                            );
                        }

                        Some(ret_map)
                    }
                    None => None,
                };

                let result = match file_size {
                    0..=UPLOAD_SIZE_LIMIT => {
                        upload_small_file(
                            &file.path,
                            &bucket_id,
                            client.clone(),
                            extra_file_info,
                            Some(move |uploaded, total, percentage, bps, eta| {
                                if let Some(callback_func) = callback.as_ref() {
                                    callback_func(
                                        &inner_file_path.clone(),
                                        uploaded,
                                        total,
                                        percentage,
                                        bps,
                                        eta,
                                    );
                                }
                            }),
                        )
                        .await
                    }
                    _ => {
                        upload_large_file(
                            &file.path,
                            &bucket_id,
                            client.clone(),
                            extra_file_info,
                            Some(move |uploaded, total, percentage, bps, eta| {
                                if let Some(callback_func) = callback.as_ref() {
                                    callback_func(
                                        &inner_file_path.clone(),
                                        uploaded,
                                        total,
                                        percentage,
                                        bps,
                                        eta,
                                    );
                                }
                            }),
                        )
                        .await
                    }
                };

                if retry_count < MAX_RETRIES {
                    match result.is_ok() {
                        true => return result,
                        false => {
                            retry_count += 1;
                            tokio::time::sleep(Duration::from_millis(750)).await;
                        }
                    };
                } else {
                    let read_handles = match task_abort_handles.read() {
                        Ok(handle) => handle,
                        Err(err) => {
                            return Err(Box::<dyn Error + Send + Sync>::from(format!(
                                "failed to get abort handles to read: {}",
                                err.to_string()
                            )))
                        }
                    };

                    for handle in read_handles.iter() {
                        handle.abort();
                    }
                }
            }
        });

        let mut writeable_abort_handles = abort_handles.write().map_err(|err| {
            Box::<dyn Error + Send + Sync>::from(format!(
                "failed to get write lock: {}",
                err.to_string()
            ))
        })?;

        writeable_abort_handles.push(join_handle.abort_handle());
        join_handles.push(join_handle);
    }

    let mut results: Vec<DynamicResult<B2File>> = Vec::new();

    for handle in join_handles.iter_mut() {
        let result = handle.await?;

        results.push(result);
    }

    Ok(results)
}

async fn upload_small_file<F>(
    file_path: &str,
    bucket_id: &str,
    client: Arc<Mutex<B2Client>>,
    optional_info: Option<HashMap<String, serde_json::Value>>,
    on_chunk: Option<F>,
) -> DynamicResult<B2File>
where
    F: Fn(u64, u64, f32, u64, u64) + Send + Sync + 'static,
{
    let path = Path::new(file_path);
    let mut file = tokio::fs::File::open(path).await?;

    let file_metadata = file.metadata().await?;
    let mut uploaded: u64 = 0;
    let file_size = file_metadata.len();

    let file_path_prefix = env::var("B2_BUCKET_PATH_PREFIX").unwrap_or_else(|_| String::new());

    let file_name = match path.file_name() {
        Some(name) => file_path_prefix + "/" + &name.to_string_lossy(),
        None => return Err(Box::from("Given file path is a folder.")),
    };

    let file_name = file_name.to_string();
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).await?;

    let sha1 = sha1_smol::Sha1::from(&buffer).digest().to_string();
    drop(buffer);

    let upload_url_req_body = B2GetUploadUrlBody::builder().bucket_id(bucket_id).build();
    let upload_url_response = client
        .lock()
        .await
        .get_upload_url(upload_url_req_body)
        .await?;

    let b2_upload_headers = B2UploadFileHeaders::builder()
        .authorization(upload_url_response.authorization_token)
        .file_name(urlencoding::encode(&file_name).into_owned())
        .content_type("b2/x-auto".into())
        .content_length(file_size as u32)
        .content_sha1(sha1)
        .build();

    let stream_file = tokio::fs::File::open(path).await?;
    let start_time = std::time::Instant::now();

    let stream = FramedRead::new(stream_file, BytesCodec::new()).inspect_ok(move |chunk| {
        uploaded += chunk.len() as u64;

        let mut elapsed_time = start_time.elapsed().as_secs();
        if elapsed_time == 0 {
            elapsed_time = 1;
        }

        let mut bytes_per_sec = uploaded / elapsed_time;
        if bytes_per_sec == 0 {
            bytes_per_sec = 1;
        }

        let eta = (file_size - uploaded) / bytes_per_sec;

        if let Some(callback) = &on_chunk {
            callback(
                uploaded,
                file_size,
                uploaded as f32 / file_size as f32,
                bytes_per_sec,
                eta,
            );
        }
    });

    let file = client
        .lock()
        .await
        .upload_file(
            b2_upload_headers,
            backblaze_b2_client::reqwest::Body::wrap_stream(stream),
            upload_url_response.upload_url,
            optional_info,
        )
        .await?;

    Ok(file)
}

#[allow(clippy::too_many_arguments)]
async fn upload_large_file_task<F>(
    client: Arc<Mutex<B2Client>>,
    file_id: String,
    task_chunk: Vec<((u64, u64), u16)>,
    file: &mut File,
    sha1s: Arc<Mutex<Vec<String>>>,
    total_uploaded: Arc<AtomicU64>,
    on_chunk: Arc<Option<F>>,
    start_time: Instant,
    file_size: u64,
) -> DynamicResult<()>
where
    F: Fn(u64, u64, f32, u64, u64) + Send + Sync + 'static,
{
    let mut upload_part_url_response = match client
        .lock()
        .await
        .get_upload_part_url(file_id.clone())
        .await
    {
        Ok(resp) => resp,
        Err(err) => return Err::<(), Box<dyn Error + Send + Sync>>(Box::new(err)),
    };

    for ((start, end), part_number) in task_chunk {
        let mut buffer = vec![0u8; (end - start) as usize];
        file.seek(std::io::SeekFrom::Start(start))?;
        file.read_exact(&mut buffer)?;

        let sha1 = sha1_smol::Sha1::from(&buffer).digest().to_string();

        let mut mutex_guard = sha1s.lock().await;
        mutex_guard[(part_number - 1) as usize] = sha1.clone();
        drop(mutex_guard);

        let chunks: Arc<Vec<Vec<u8>>> =
            Arc::new(buffer.chunks(8192 * 10).map(|s| s.into()).collect());
        drop(buffer);

        loop {
            let chunks = chunks.clone();
            let total_uploaded = total_uploaded.clone();
            let sha1 = sha1.clone();
            let upload_part_headers = B2UploadPartHeaders::builder()
                .authorization(upload_part_url_response.authorization_token.clone())
                .part_number(part_number)
                .content_length(u32::try_from(end - start).unwrap())
                .content_sha1(sha1)
                .build();

            let on_chunk = on_chunk.clone();
            let mut total_uploaded_here: u64 = 0;
            let total_uploaded_other = total_uploaded.clone();
            let stream = async_stream::stream! {
                for out in chunks.iter() {
                    let total = total_uploaded.fetch_add(out.len() as u64, atomic::Ordering::Relaxed);
                    let total = total + out.len() as u64;
                    *(&mut total_uploaded_here) += out.len() as u64;

                    let mut elapsed_time = start_time.elapsed().as_secs();
                    if elapsed_time == 0 {
                        elapsed_time = 1;
                    }

                    let mut bytes_per_sec = total / elapsed_time;
                    if bytes_per_sec == 0 {
                        bytes_per_sec = 1;
                    }

                    let eta = (file_size - total) / bytes_per_sec;

                    if let Some(callback) = on_chunk.as_ref() {
                        callback(total, file_size, total as f32 / file_size as f32, bytes_per_sec, eta);
                    }

                    yield Ok::<_, Infallible>(out.clone());
                }
            };

            let stream = backblaze_b2_client::reqwest::Body::wrap_stream(stream);

            let result = client
                .lock()
                .await
                .upload_part(
                    upload_part_headers,
                    stream,
                    upload_part_url_response.upload_url.clone(),
                )
                .await;

            if let Err(error) = result {
                if let B2BasicError::RequestError(error) = error {
                    if error.status == nonzero!(503u16) {
                        upload_part_url_response = match client
                            .lock()
                            .await
                            .get_upload_part_url(file_id.clone())
                            .await
                        {
                            Ok(resp) => resp,
                            Err(err) => return Err(Box::new(err)),
                        };

                        total_uploaded_other
                            .fetch_sub(total_uploaded_here, atomic::Ordering::Relaxed);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    } else {
                        return Err(Box::new(B2BasicError::RequestError(error)));
                    }
                } else {
                    return Err(Box::new(error));
                }
            } else {
                break;
            }
        }
    }

    Ok(())
}

async fn upload_large_file<F>(
    file_path: &str,
    bucket_id: &str,
    client: Arc<Mutex<B2Client>>,
    optional_info: Option<HashMap<String, serde_json::Value>>,
    on_chunk: Option<F>,
) -> DynamicResult<B2File>
where
    F: Fn(u64, u64, f32, u64, u64) + Send + Sync + 'static,
{
    let path = Path::new(file_path);
    let file = File::open(path)?;

    let file_metadata = file.metadata()?;

    let file_path_prefix = env::var("B2_BUCKET_PATH_PREFIX").unwrap_or_else(|_| String::new());

    let file_name = match path.file_name() {
        Some(name) => file_path_prefix + "/" + &name.to_string_lossy(),
        None => return Err(Box::from("Given file path is a folder.")),
    };

    let file_name = file_name.to_string();

    let start_large_upload_body = B2StartLargeFileUploadBody::builder()
        .bucket_id(bucket_id.into())
        .file_name(file_name)
        .content_type("b2/x-auto".into())
        .file_info(optional_info)
        .build();

    let start_large_file_response = match client
        .lock()
        .await
        .start_large_file(start_large_upload_body)
        .await
    {
        Ok(resp) => resp,
        Err(err) => return Err(Box::new(err)),
    };

    let file_id = start_large_file_response.file_id;
    let file_size = file_metadata.len();
    let total_uploaded = Arc::new(atomic::AtomicU64::new(0));

    let mut parts: Vec<((u64, u64), u16)> = vec![];

    let mut current_range_start: u16 = 0;

    const CHUNK_SIZE: u32 = 20;

    loop {
        let start = u64::from((MEGABYTE * CHUNK_SIZE) * u32::from(current_range_start));
        let end = u64::from((MEGABYTE * CHUNK_SIZE) * (u32::from(current_range_start) + 1));

        current_range_start += 1;

        if end >= file_size {
            parts.push(((start, file_size), current_range_start));
            break;
        } else {
            parts.push(((start, end), current_range_start));
        }
    }

    let sha1s: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![String::from(""); parts.len()]));
    let mut join_handles: Vec<JoinHandle<DynamicResult>> = vec![];
    let abort_handles: Arc<RwLock<Vec<AbortHandle>>> = Arc::new(RwLock::new(vec![]));
    let on_chunk = Arc::new(on_chunk);
    let start_time = std::time::Instant::now();

    for chunk in parts.chunks(10) {
        let task_chunk = chunk.to_owned();
        let client = client.clone();
        let file_id = file_id.clone();
        let sha1s = sha1s.clone();
        let mut file = match File::open(file_path) {
            Ok(file) => file,
            Err(err) => return Err(Box::new(err)),
        };

        let task_abort_handles = abort_handles.clone();
        let total_uploaded = total_uploaded.clone();
        let on_chunk = on_chunk.clone();

        let join_handle = tokio::spawn(async move {
            let result = upload_large_file_task(
                client.clone(),
                file_id,
                task_chunk,
                &mut file,
                sha1s.clone(),
                total_uploaded.clone(),
                on_chunk.clone(),
                start_time,
                file_size,
            )
            .await;

            if let Err(err) = result {
                for handle in task_abort_handles.read().unwrap().iter() {
                    handle.abort();
                }

                return Err(err);
            }

            Ok(())
        });

        let abort_handle = join_handle.abort_handle();

        join_handles.push(join_handle);
        abort_handles.write().unwrap().push(abort_handle);
    }

    for handle in join_handles {
        match handle.await {
            Ok(res) => res,
            Err(err) => match err.is_cancelled() {
                true => continue,
                false => return Err(Box::new(err)),
            },
        }?;
    }

    let result = client
        .lock()
        .await
        .finish_large_file(
            B2FinishLargeFileBody::builder()
                .file_id(file_id.clone())
                .part_sha1_array(sha1s.lock().await.to_vec())
                .build(),
        )
        .await;

    match result {
        Ok(res) => Ok(res),
        Err(err) => Err(Box::new(err)),
    }
}

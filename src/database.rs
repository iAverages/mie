use crate::types::AddMedia;

use sqlx::MySqlPool;

pub struct DB {
    connection_url: String,
    pool: MySqlPool,
}

impl DB {
    pub async fn new(connection_url: &str) -> Result<DB, sqlx::Error> {
        let pool = MySqlPool::connect(connection_url).await?;

        Ok(DB {
            connection_url: connection_url.to_string(),
            pool,
        })
    }

    pub fn get_all(&self) -> Vec<String> {
        vec!["gay".to_string()]
    }

    pub async fn add(&self, media: AddMedia) -> u64 {
        let res = sqlx::query!(
            "INSERT INTO media 
        (url, actual_source, original_source, size, type, meta, uploader)
        VALUES (?,?,?,?,?,?,?)",
            media.url,
            media.actual_source,
            media.original_source,
            media.size,
            media.file_type,
            media.meta,
            media.uploader,
        )
        .execute(&self.pool)
        .await
        .unwrap();

        println!("Adding media: {:?}", res);

        res.last_insert_id()
    }
}

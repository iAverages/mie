use base64::{engine::general_purpose, Engine as _};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Method, RequestBuilder, Response,
};
use serde::{de::DeserializeOwned, Serialize, Serializer};
use std::{collections::HashMap, num::NonZeroU16, str::FromStr};

use crate::structs::{
    B2BasicError, B2BasicErrorBody, B2Client, B2File, B2FilePart, B2FinishLargeFileBody,
    B2GetUploadPartUrlResponse, B2GetUploadUrlBody, B2GetUploadUrlResponse, B2KeyCapabilities,
    B2ListFilesBody, B2ListFilesResponse, B2StartLargeFileUploadBody,
    B2StartLargeFileUploadResponse, B2UpdateFileRetentionBody, B2UpdateFileRetentionResponse,
    B2UploadFileHeaders, B2UploadPartHeaders,
};

impl B2Client {
    pub async fn new(key_id: &str, application_key: &str) -> Result<B2Client, B2BasicError> {
        let auth_token = format!(
            "Basic {}",
            general_purpose::STANDARD_NO_PAD.encode(format!("{}:{}", key_id, application_key))
        );

        let reqwest_client = reqwest::Client::new();

        let auth_response = reqwest_client
            .get("https://api.backblazeb2.com/b2api/v2/b2_authorize_account")
            .header("Authorization", auth_token)
            .send()
            .await;

        Ok(B2Client {
            reqwest_client,
            auth_data: B2Client::handle_response(auth_response).await?,
        })
    }

    pub async fn authorize_account(
        &mut self,
        key_id: &str,
        application_key: &str,
    ) -> Result<(), B2BasicError> {
        let auth_token = format!(
            "Basic {}",
            general_purpose::STANDARD_NO_PAD.encode(format!("{}:{}", key_id, application_key))
        );

        let reqwest_client = reqwest::Client::new();

        let auth_response = reqwest_client
            .get("https://api.backblazeb2.com/b2api/v2/b2_authorize_account")
            .header("Authorization", auth_token)
            .send()
            .await;

        self.auth_data = B2Client::handle_response(auth_response).await?;
        Ok(())
    }

    pub async fn list_file_names(
        &self,
        request_body: B2ListFilesBody,
    ) -> Result<B2ListFilesResponse, B2BasicError> {
        let response = self
            .create_request_with_token(Method::POST, "b2_list_file_names", None)
            .json(&request_body)
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub async fn get_upload_part_url(
        &self,
        file_id: String,
    ) -> Result<B2GetUploadPartUrlResponse, B2BasicError> {
        let response = self
            .create_request_with_token(Method::GET, "b2_get_upload_part_url", None)
            .query(&[("fileId", file_id)])
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub async fn get_upload_url(
        &self,
        request_body: B2GetUploadUrlBody,
    ) -> Result<B2GetUploadUrlResponse, B2BasicError> {
        let response = self
            .create_request_with_token(Method::POST, "b2_get_upload_url", None)
            .json(&request_body)
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub async fn start_large_file(
        &self,
        request_body: B2StartLargeFileUploadBody,
    ) -> Result<B2StartLargeFileUploadResponse, B2BasicError> {
        let response = self
            .create_request_with_token(Method::POST, "b2_start_large_file", None)
            .json(&request_body)
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub async fn upload_file<S: AsRef<str>>(
        &self,
        request_headers: B2UploadFileHeaders,
        file: impl Into<reqwest::Body>,
        upload_url: String,
        file_info: Option<HashMap<S, serde_json::Value>>,
    ) -> Result<B2File, B2BasicError> {
        let file_info = match file_info {
            Some(map) => map,
            None => HashMap::new(),
        };

        let file_info: HashMap<_, _> = file_info
            .iter()
            .map(|(key, value)| {
                let key_ref = key.as_ref();
                (format!("X-Bz-Info-{key_ref}"), value)
            })
            .collect();

        let response = self
            .reqwest_client
            .request(Method::POST, upload_url)
            .headers(request_headers.into())
            .headers(hash_map_to_headers(file_info))
            .body(file)
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub async fn upload_part(
        &self,
        request_headers: B2UploadPartHeaders,
        part: impl Into<reqwest::Body>,
        upload_url: String,
    ) -> Result<B2FilePart, B2BasicError> {
        let response = self
            .reqwest_client
            .request(Method::POST, upload_url)
            .headers(request_headers.into())
            .body(part)
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub async fn finish_large_file(
        &self,
        request_body: B2FinishLargeFileBody,
    ) -> Result<B2File, B2BasicError> {
        let response = self
            .create_request_with_token(Method::POST, "b2_finish_large_file", None)
            .json(&request_body)
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub async fn update_file_retention(
        &self,
        request_body: B2UpdateFileRetentionBody,
    ) -> Result<B2UpdateFileRetentionResponse, B2BasicError> {
        let response = self
            .create_request_with_token(Method::POST, "b2_update_file_retention", None)
            .json(&request_body)
            .send()
            .await;

        B2Client::handle_response(response).await
    }

    pub fn has_permission(&self, permission: &B2KeyCapabilities) -> bool {
        self.auth_data.allowed.capabilities.contains(permission)
    }

    pub fn has_all_permissions(&self, permissions: &Vec<B2KeyCapabilities>) -> bool {
        permissions
            .iter()
            .all(|permission| self.has_permission(permission))
    }

    pub fn get_authorization_token(&self) -> &str {
        &self.auth_data.authorization_token
    }

    fn create_request(&self, api_name: &str) -> String {
        format!("{}/b2api/v2/{}", self.auth_data.api_url, api_name)
    }

    fn create_request_with_token(
        &self,
        method: Method,
        api_name: &str,
        alt_url: Option<String>,
    ) -> RequestBuilder {
        let url = match alt_url {
            Some(url) => url,
            None => self.create_request(api_name),
        };

        self.reqwest_client
            .request(method, url)
            .header("Authorization", self.get_authorization_token())
    }

    async fn response_option_handling(
        response: Result<Response, reqwest::Error>,
    ) -> Result<Response, B2BasicError> {
        let response = match response {
            Ok(resp) => resp,
            Err(error) => return Err(B2BasicError::RequestSendError(error.to_string())),
        };

        let response_code = u16::from(response.status());
        if response_code >= 400 {
            let error_json: B2BasicErrorBody = match response.json().await {
                Ok(json) => json,
                Err(_) => B2BasicErrorBody {
                    status: NonZeroU16::new(response_code).unwrap(),
                    code: String::from(""),
                    message: String::from(""),
                },
            };

            return Err(B2BasicError::RequestError(error_json));
        };

        Ok(response)
    }

    async fn handle_response<T: DeserializeOwned>(
        response: Result<Response, reqwest::Error>,
    ) -> Result<T, B2BasicError> {
        let response = match B2Client::response_option_handling(response).await {
            Ok(resp) => resp,
            Err(error) => return Err(error),
        };

        let text = response.text().await.unwrap();

        match serde_json::from_str::<T>(&text) {
            Ok(json) => Ok(json),
            Err(error) => Err(B2BasicError::JsonParseError(error.to_string())),
        }
    }
}

impl Serialize for B2KeyCapabilities {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

fn hash_map_to_headers(map: HashMap<String, &serde_json::Value>) -> HeaderMap {
    map.iter()
        .map(|(name, value)| {
            let v = match value {
                serde_json::Value::Null => "".into(),
                serde_json::Value::String(str) => str.clone(),
                _ => value.to_string(),
            };

            (HeaderName::from_str(name), HeaderValue::from_str(&v))
        })
        .filter(|(k, v)| k.is_ok() && v.is_ok())
        .filter(|(_, v)| !v.as_ref().unwrap().is_empty())
        .map(|(k, v)| (k.unwrap(), v.unwrap()))
        .collect()
}
